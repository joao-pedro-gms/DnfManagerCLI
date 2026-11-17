//! Detecção automática de categorias e tags a partir de metadados do sistema.
//!
//! Quatro fontes, em ordem de confiança:
//!
//! 1. **AppStream** — categorias funcionais oficiais (catálogo do swcatalog);
//! 2. **desktop files** — categorias freedesktop de quem não está no catálogo;
//! 3. **lista de arquivos** — tipo do pacote (comando, biblioteca, doc…);
//! 4. **domínio da URL** — família de projeto (tug.org → TeX Live).
//!
//! Nada aqui pede nada ao usuário: a camada própria continua sendo apenas um
//! override opcional por cima.

mod appstream;
mod domains;
mod files;

pub use appstream::{parse_catalog, parse_catalog_file};
pub use domains::family_from_url;
pub use files::{desktop_categories, parse_desktop_file, signals_from_paths, PackageFiles};

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::model::Package;

/// De onde veio uma categoria ou tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagSource {
    /// Categoria própria do usuário.
    Custom,
    /// AppStream (metadado oficial do projeto).
    AppStream,
    /// Arquivo `.desktop` instalado.
    DesktopFile,
    /// Regra curada por prefixo/palavra-chave.
    Curated,
    /// Domínio da URL do pacote.
    UrlDomain,
    /// Inferência a partir dos arquivos instalados.
    FileSignal,
}

impl TagSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "própria",
            Self::AppStream => "appstream",
            Self::DesktopFile => "desktop",
            Self::Curated => "curada",
            Self::UrlDomain => "url",
            Self::FileSignal => "arquivos",
        }
    }

    pub fn confidence(self) -> u8 {
        match self {
            Self::Custom => 100,
            Self::AppStream => 90,
            Self::DesktopFile => 75,
            Self::Curated => 60,
            Self::UrlDomain => 50,
            Self::FileSignal => 40,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Custom => "custom",
            Self::AppStream => "appstream",
            Self::DesktopFile => "desktop",
            Self::Curated => "curated",
            Self::UrlDomain => "url",
            Self::FileSignal => "files",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw {
            "custom" => Self::Custom,
            "appstream" => Self::AppStream,
            "desktop" => Self::DesktopFile,
            "url" => Self::UrlDomain,
            "files" => Self::FileSignal,
            _ => Self::Curated,
        }
    }
}

/// Uma categoria/tag com procedência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub label: String,
    pub source: TagSource,
}

impl Tag {
    pub fn new(label: impl Into<String>, source: TagSource) -> Self {
        Self {
            label: label.into(),
            source,
        }
    }

    pub fn confidence(&self) -> u8 {
        self.source.confidence()
    }
}

/// Sinais de tipo derivados da lista de arquivos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileSignals(u16);

impl FileSignals {
    pub const BIN: u16 = 1 << 0;
    pub const LIB: u16 = 1 << 1;
    pub const INCLUDE: u16 = 1 << 2;
    pub const MAN: u16 = 1 << 3;
    pub const DOC: u16 = 1 << 4;
    pub const FONTS: u16 = 1 << 5;
    pub const SYSTEMD: u16 = 1 << 6;
    pub const DESKTOP: u16 = 1 << 7;
    pub const METAINFO: u16 = 1 << 8;
    pub const I18N: u16 = 1 << 9;
    pub const PYTHON: u16 = 1 << 10;
    pub const PERL: u16 = 1 << 11;
    pub const CONFIG: u16 = 1 << 12;
    pub const HEADERS: u16 = 1 << 13;

    pub fn new(bits: u16) -> Self {
        Self(bits)
    }

    pub fn bits(self) -> u16 {
        self.0
    }

    pub fn set(&mut self, bit: u16) {
        self.0 |= bit;
    }

    pub fn has(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    /// Rótulos legíveis dos sinais ativos, na ordem de exibição.
    pub fn labels(self) -> Vec<&'static str> {
        [
            (Self::BIN, "comando"),
            (Self::LIB, "biblioteca"),
            (Self::INCLUDE, "cabeçalhos"),
            (Self::MAN, "man"),
            (Self::DOC, "documentação"),
            (Self::FONTS, "fontes"),
            (Self::SYSTEMD, "serviço"),
            (Self::DESKTOP, "aplicativo"),
            (Self::METAINFO, "appstream"),
            (Self::I18N, "tradução"),
            (Self::PYTHON, "python"),
            (Self::PERL, "perl"),
            (Self::CONFIG, "configuração"),
            (Self::HEADERS, "desenvolvimento"),
        ]
        .into_iter()
        .filter(|(bit, _)| self.has(*bit))
        .map(|(_, label)| label)
        .collect()
    }
}

/// Componente AppStream relevante para um pacote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStreamComponent {
    pub id: String,
    pub categories: Vec<String>,
    pub summary: Option<String>,
}

impl AppStreamComponent {
    /// Categoria principal, pela ordem de prioridade do freedesktop.
    pub fn primary_category(&self) -> Option<&str> {
        const PRIORITY: &[&str] = &[
            "AudioVideo",
            "Audio",
            "Video",
            "Graphics",
            "Development",
            "Office",
            "Network",
            "Game",
            "Education",
            "Science",
            "System",
            "Settings",
            "Utility",
            "Addons",
            "Fonts",
        ];
        PRIORITY
            .iter()
            .find(|want| self.categories.iter().any(|c| c == *want))
            .copied()
            .or_else(|| self.categories.first().map(String::as_str))
    }
}

/// Resultado de uma leitura de arquivos + URLs do sistema.
#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    pub files: HashMap<String, PackageFiles>,
    /// pacote → família detectada pelo domínio da URL
    pub families: HashMap<String, String>,
}

/// Índice completo de metadados automáticos.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagCatalog {
    /// pacote → componente AppStream
    pub appstream: HashMap<String, AppStreamComponent>,
    /// pacote → categorias dos seus .desktop
    pub desktop: HashMap<String, Vec<String>>,
    /// pacote → arquivos agregados
    pub files: HashMap<String, PackageFiles>,
    /// pacote → família de projeto deduzida da URL
    pub families: HashMap<String, String>,
}

impl TagCatalog {
    pub fn is_empty(&self) -> bool {
        self.appstream.is_empty()
            && self.desktop.is_empty()
            && self.files.is_empty()
            && self.families.is_empty()
    }

    pub fn len(&self) -> usize {
        self.appstream.len() + self.desktop.len() + self.files.len()
    }

    /// Monta o catálogo a partir do sistema: AppStream + .desktop + arquivos.
    pub fn build_system(index: FileIndex) -> Self {
        let mut catalog = TagCatalog {
            files: index.files,
            families: index.families,
            ..Default::default()
        };
        for dir in appstream::default_catalog_dirs() {
            catalog.appstream.extend(appstream::parse_catalog_dir(&dir));
        }
        catalog.load_desktop(std::path::Path::new("/usr/share/applications"));
        catalog
    }

    /// Preenche as categorias dos `.desktop` a partir dos ids detectados.
    pub fn load_desktop(&mut self, dir: &std::path::Path) {
        let mut resolvidos: Vec<(String, Vec<String>)> = Vec::new();
        for (pacote, files) in &self.files {
            if files.desktop_ids.is_empty() {
                continue;
            }
            let categorias = files::desktop_categories(dir, &files.desktop_ids);
            if !categorias.is_empty() {
                resolvidos.push((pacote.clone(), categorias));
            }
        }
        for (pacote, categorias) in resolvidos {
            self.desktop.insert(pacote, categorias);
        }
    }

    pub fn file_info(&self, name: &str) -> Option<&PackageFiles> {
        self.files.get(name)
    }

    /// Família de projeto: domínio da URL tem prioridade sobre o prefixo.
    pub fn family_for(&self, name: &str) -> Option<&str> {
        self.families.get(name).map(String::as_str)
    }

    pub fn appstream_for(&self, name: &str) -> Option<&AppStreamComponent> {
        self.appstream.get(name)
    }

    /// Categoria funcional de um pacote, com procedência.
    pub fn category_for(&self, pkg: &Package) -> Option<Tag> {
        if let Some(component) = self.appstream.get(&pkg.name) {
            if let Some(primary) = component.primary_category() {
                return Some(Tag::new(category_label(primary), TagSource::AppStream));
            }
        }
        if let Some(categories) = self.desktop.get(&pkg.name) {
            if let Some(primary) = categories
                .iter()
                .find(|c| is_main_category(c))
                .or_else(|| categories.first())
            {
                return Some(Tag::new(category_label(primary), TagSource::DesktopFile));
            }
        }
        None
    }

    /// Tags multi-valoradas: categorias AppStream + tipo + família por URL.
    pub fn tags_for(&self, pkg: &Package) -> Vec<Tag> {
        let mut tags: Vec<Tag> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(component) = self.appstream.get(&pkg.name) {
            for category in &component.categories {
                let label = category_label(category);
                if seen.insert(label.clone()) {
                    tags.push(Tag::new(label, TagSource::AppStream));
                }
            }
        } else if let Some(categories) = self.desktop.get(&pkg.name) {
            for category in categories {
                let label = category_label(category);
                if seen.insert(label.clone()) {
                    tags.push(Tag::new(label, TagSource::DesktopFile));
                }
            }
        }

        if let Some(files) = self.files.get(&pkg.name) {
            for label in files.signals.labels() {
                if seen.insert(label.to_string()) {
                    tags.push(Tag::new(label, TagSource::FileSignal));
                }
            }
        }

        tags
    }
}

/// Categorias principais do freedesktop, usadas para escolher a primária.
pub fn is_main_category(name: &str) -> bool {
    matches!(
        name,
        "AudioVideo"
            | "Audio"
            | "Video"
            | "Development"
            | "Education"
            | "Game"
            | "Graphics"
            | "Network"
            | "Office"
            | "Science"
            | "Settings"
            | "System"
            | "Utility"
    )
}

/// Traduz a categoria AppStream para o rótulo usado na interface.
pub fn category_label(raw: &str) -> String {
    match raw {
        "AudioVideo" | "Audio" | "Video" | "Player" | "Recorder" | "AudioVideoEditing" => {
            "Multimídia".to_string()
        }
        "Graphics" | "2DGraphics" | "RasterGraphics" | "VectorGraphics" | "3DGraphics" => {
            "Gráficos".to_string()
        }
        "Development" | "IDE" | "Debugger" | "Profiling" | "RevisionControl" => {
            "Desenvolvimento".to_string()
        }
        "Office" | "WordProcessor" | "Spreadsheet" | "Presentation" | "Calculator" => {
            "Escritório".to_string()
        }
        "Network" | "WebBrowser" | "Email" | "InstantMessaging" | "FileTransfer" => {
            "Rede".to_string()
        }
        "Game" | "Emulator" | "ActionGame" | "AdventureGame" | "LogicGame" => "Jogos".to_string(),
        "Education" | "Science" | "Math" => "Educação e ciência".to_string(),
        "System" | "PackageManager" | "FileTools" | "FileManager" => "Sistema".to_string(),
        "Settings" | "DesktopSettings" | "HardwareSettings" => "Configurações".to_string(),
        "Utility" | "Compression" | "TextEditor" => "Utilitários".to_string(),
        "Fonts" => "Fontes".to_string(),
        "Addons" | "Plugin" => "Complementos".to_string(),
        "Viewer" => "Visualizador".to_string(),
        "Security" => "Segurança".to_string(),
        other => other.to_string(),
    }
}

/// Contagem de tags para estatísticas.
pub fn tag_counts(packages: &[Package], catalog: &TagCatalog) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for pkg in packages {
        for tag in catalog.tags_for(pkg) {
            *counts.entry(tag.label).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests;
