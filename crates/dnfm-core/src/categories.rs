//! Categorias e famílias de pacotes.
//!
//! No Fedora atual o tag `Group` do RPM está morto (quase tudo "Unspecified") e
//! o comps cobre uma fração pequena da base instalada. A taxonomia aqui é:
//!
//! 1. **categoria própria** — o que o usuário atribuiu, persistido em XDG;
//! 2. **categoria curada** — regras por prefixo/palavra-chave (grossa);
//! 3. **família** — prefixo do nome, com apelidos que juntam parentes
//!    (`kf5`, `kf6`, `plasma` viram "KDE Plasma").

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::TagCatalog;
use crate::model::Package;

/// Modo de agrupamento da listagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupMode {
    #[default]
    None,
    Category,
    Family,
    Repo,
    Reason,
}

impl GroupMode {
    pub const ALL: [GroupMode; 5] = [
        GroupMode::None,
        GroupMode::Category,
        GroupMode::Family,
        GroupMode::Repo,
        GroupMode::Reason,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "sem grupo",
            Self::Category => "categoria",
            Self::Family => "família",
            Self::Repo => "repo",
            Self::Reason => "motivo",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Category => "category",
            Self::Family => "family",
            Self::Repo => "repo",
            Self::Reason => "reason",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "none" | "nenhum" => Some(Self::None),
            "category" | "categoria" => Some(Self::Category),
            "family" | "familia" | "família" => Some(Self::Family),
            "repo" | "repositorio" | "repositório" => Some(Self::Repo),
            "reason" | "motivo" => Some(Self::Reason),
            _ => None,
        }
    }
}

/// Apelidos de prefixo: juntam pacotes que pertencem ao mesmo projeto.
const ALIASES: &[(&str, &str)] = &[
    ("texlive", "TeX Live"),
    ("tex", "TeX Live"),
    ("perl", "Perl"),
    ("python", "Python"),
    ("python3", "Python"),
    ("python2", "Python"),
    ("pypy", "Python"),
    ("kf5", "KDE Plasma"),
    ("kf6", "KDE Plasma"),
    ("plasma", "KDE Plasma"),
    ("kwin", "KDE Plasma"),
    ("kde", "KDE Plasma"),
    ("qt5", "Qt"),
    ("qt6", "Qt"),
    ("qt", "Qt"),
    ("gnome", "GNOME"),
    ("gtk", "GNOME"),
    ("gsettings", "GNOME"),
    ("wine", "Wine"),
    ("java", "Java"),
    ("openjdk", "Java"),
    ("jdk", "Java"),
    ("jre", "Java"),
    ("maven", "Java"),
    ("golang", "Go"),
    ("go", "Go"),
    ("rust", "Rust"),
    ("cargo", "Rust"),
    ("nodejs", "Node.js"),
    ("npm", "Node.js"),
    ("node", "Node.js"),
    ("php", "PHP"),
    ("ruby", "Ruby"),
    ("gem", "Ruby"),
    ("libreoffice", "LibreOffice"),
    ("autocorr", "LibreOffice"),
    ("docker", "Contêineres"),
    ("podman", "Contêineres"),
    ("buildah", "Contêineres"),
    ("skopeo", "Contêineres"),
    ("containerd", "Contêineres"),
    ("runc", "Contêineres"),
    ("crun", "Contêineres"),
    ("conmon", "Contêineres"),
    ("xorg", "Xorg"),
    ("xkeyboard", "Xorg"),
    ("mesa", "Gráficos"),
    ("vulkan", "Gráficos"),
    ("ffmpeg", "Multimídia"),
    ("gstreamer", "Multimídia"),
    ("vlc", "Multimídia"),
    ("mpv", "Multimídia"),
    ("pipewire", "Multimídia"),
    ("pulseaudio", "Multimídia"),
    ("alsa", "Multimídia"),
    ("systemd", "Sistema base"),
    ("udev", "Sistema base"),
    ("dbus", "Sistema base"),
    ("polkit", "Sistema base"),
    ("kernel", "Kernel e boot"),
    ("dracut", "Kernel e boot"),
    ("grub2", "Kernel e boot"),
    ("shim", "Kernel e boot"),
    ("selinux", "Segurança"),
    ("policycoreutils", "Segurança"),
    ("firewalld", "Segurança"),
    ("gnupg", "Segurança"),
    ("crypto", "Segurança"),
    ("openssl", "Segurança"),
];

/// Regras por palavra-chave, aplicadas ao nome (mais específicas primeiro).
const KEYWORDS: &[(&str, &str)] = &[
    ("firmware", "Firmware"),
    ("font", "Fontes"),
    ("langpack", "Idiomas"),
    ("-devel", "Desenvolvimento"),
    ("-headers", "Desenvolvimento"),
    ("-doc", "Documentação"),
    ("-debug", "Depuração"),
    ("-libs", "Bibliotecas"),
    ("-static", "Bibliotecas"),
    ("theme", "Temas"),
    ("icon", "Temas"),
    ("wallpaper", "Temas"),
    ("backgrounds", "Temas"),
    ("-plugin", "Plugins"),
];

const FALLBACK_CATEGORY: &str = "Outros";
const OTHER_FAMILY: &str = "Outros";
/// Famílias menores que isso caem em "Outros" no agrupamento por família.
const MIN_FAMILY_SIZE: usize = 3;

fn package_key(name: &str, arch: &str) -> String {
    format!("{name}.{arch}")
}

fn prefix(name: &str) -> &str {
    name.split('-').next().unwrap_or(name)
}

fn alias_for(prefix: &str) -> Option<&'static str> {
    if let Some(label) = lookup_alias(prefix) {
        return Some(label);
    }
    // `gtk3`, `gtk4`, `qt6`: o sufixo numérico não muda a família.
    let trimmed = prefix.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != prefix && !trimmed.is_empty() {
        return lookup_alias(trimmed);
    }
    None
}

fn lookup_alias(prefix: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, label)| *label)
}

/// Categoria curada de um pacote (grossa).
pub fn categorize(pkg: &Package) -> String {
    let name = pkg.name.to_ascii_lowercase();
    if let Some(label) = alias_for(prefix(&name)) {
        return label.to_string();
    }
    for (needle, label) in KEYWORDS {
        if name.contains(needle) {
            return label.to_string();
        }
    }
    if name.starts_with("lib") {
        return "Bibliotecas".to_string();
    }
    FALLBACK_CATEGORY.to_string()
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Família de um pacote (fina), a partir do prefixo do nome.
pub fn family(pkg: &Package) -> String {
    let name = pkg.name.to_ascii_lowercase();
    let prefix = prefix(&name);
    alias_for(prefix)
        .map(str::to_string)
        .unwrap_or_else(|| capitalized(prefix))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assignment {
    pub name: String,
    pub arch: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CategoryFile {
    version: u32,
    #[serde(default)]
    assignments: Vec<Assignment>,
}

const CATEGORY_VERSION: u32 = 1;

/// Categorias definidas pelo usuário, persistidas em XDG.
#[derive(Debug, Clone, Default)]
pub struct CustomCategories {
    path: PathBuf,
    items: BTreeMap<String, Assignment>,
    dirty: bool,
}

impl CustomCategories {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn default_path() -> PathBuf {
        crate::paths::state_dir().join("categories.json")
    }

    pub fn load_default() -> Self {
        Self::load(Self::default_path())
    }

    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut set = Self {
            path,
            items: BTreeMap::new(),
            dirty: false,
        };
        let Ok(raw) = fs::read_to_string(&set.path) else {
            return set;
        };
        match serde_json::from_str::<CategoryFile>(&raw) {
            Ok(file) if file.version <= CATEGORY_VERSION => {
                for item in file.assignments {
                    set.items.insert(package_key(&item.name, &item.arch), item);
                }
            }
            Ok(file) => {
                tracing::warn!(
                    version = file.version,
                    "categories.json de versão mais nova"
                );
            }
            Err(err) => {
                let backup = set.path.with_extension("json.corrupt");
                let _ = fs::rename(&set.path, &backup);
                tracing::warn!(%err, backup = %backup.display(), "categories.json inválido");
            }
        }
        set
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, name: &str, arch: &str) -> Option<&str> {
        self.items
            .get(&package_key(name, arch))
            .map(|a| a.label.as_str())
    }

    pub fn assign(&mut self, packages: &[Package], label: &str) {
        let label = label.trim();
        if label.is_empty() {
            return;
        }
        for pkg in packages {
            self.items.insert(
                package_key(&pkg.name, &pkg.arch),
                Assignment {
                    name: pkg.name.clone(),
                    arch: pkg.arch.clone(),
                    label: label.to_string(),
                },
            );
        }
        self.dirty = true;
    }

    pub fn unassign(&mut self, name: &str, arch: &str) -> bool {
        let removed = self.items.remove(&package_key(name, arch)).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.dirty = true;
        }
    }

    pub fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self.items.values().map(|a| a.label.clone()).collect();
        labels.sort();
        labels.dedup();
        labels
    }

    pub fn iter(&self) -> impl Iterator<Item = &Assignment> {
        self.items.values()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Remove atribuições de pacotes que não estão instalados.
    pub fn prune(&mut self, installed: &[&str]) -> usize {
        let before = self.items.len();
        self.items
            .retain(|_, a| installed.iter().any(|n| *n == a.name));
        let removed = before - self.items.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = CategoryFile {
            version: CATEGORY_VERSION,
            assignments: self.items.values().cloned().collect(),
        };
        let body = serde_json::to_vec_pretty(&file)?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut fh = fs::File::create(&tmp)?;
            fh.write_all(&body)?;
            fh.write_all(b"\n")?;
            fh.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }
}

/// Uma categoria visível na interface, com contagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryInfo {
    pub label: String,
    pub count: usize,
    pub custom: bool,
}

/// Índice de categorias e famílias para um conjunto de pacotes.
#[derive(Debug, Clone, Default)]
pub struct Taxonomy {
    categories: Vec<CategoryInfo>,
    category_of: HashMap<String, String>,
    family_of: HashMap<String, String>,
}

impl Taxonomy {
    pub fn build(packages: &[Package], custom: &CustomCategories) -> Self {
        Self::build_with(packages, custom, None)
    }

    /// Como [`Self::build`], mas usando o catálogo automático (AppStream,
    /// .desktop e domínio da URL) antes de cair nas regras curadas.
    pub fn build_with(
        packages: &[Package],
        custom: &CustomCategories,
        catalog: Option<&TagCatalog>,
    ) -> Self {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut custom_labels: BTreeMap<String, usize> = BTreeMap::new();
        let mut category_of = HashMap::with_capacity(packages.len());
        let mut family_of = HashMap::with_capacity(packages.len());

        let mut family_counts: BTreeMap<String, usize> = BTreeMap::new();
        for pkg in packages {
            family_counts
                .entry(family(pkg))
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        for pkg in packages {
            let key = package_key(&pkg.name, &pkg.arch);
            let label = match custom.get(&pkg.name, &pkg.arch) {
                Some(label) => {
                    custom_labels
                        .entry(label.to_string())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    label.to_string()
                }
                None => catalog
                    .and_then(|c| c.category_for(pkg))
                    .map(|tag| tag.label)
                    .unwrap_or_else(|| categorize(pkg)),
            };
            counts
                .entry(label.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            category_of.insert(key.clone(), label);

            let name = pkg.name.to_ascii_lowercase();
            // 1) domínio da URL, quando identifica o projeto; 2) apelido do
            // prefixo; 3) prefixo com limiar de tamanho.
            let fam = match catalog.and_then(|c| c.family_for(&pkg.name)) {
                Some(label) => label.to_string(),
                None => match alias_for(prefix(&name)) {
                    Some(label) => label.to_string(),
                    None => {
                        let raw = family(pkg);
                        if family_counts.get(&raw).copied().unwrap_or(0) < MIN_FAMILY_SIZE {
                            OTHER_FAMILY.to_string()
                        } else {
                            raw
                        }
                    }
                },
            };
            family_of.insert(key, fam);
        }

        let categories = counts
            .into_iter()
            .map(|(label, count)| CategoryInfo {
                custom: custom_labels.contains_key(&label),
                label,
                count,
            })
            .collect();

        Self {
            categories,
            category_of,
            family_of,
        }
    }

    pub fn categories(&self) -> &[CategoryInfo] {
        &self.categories
    }

    pub fn category_of(&self, pkg: &Package) -> &str {
        self.category_of
            .get(&package_key(&pkg.name, &pkg.arch))
            .map(String::as_str)
            .unwrap_or(FALLBACK_CATEGORY)
    }

    pub fn family_of(&self, pkg: &Package) -> &str {
        self.family_of
            .get(&package_key(&pkg.name, &pkg.arch))
            .map(String::as_str)
            .unwrap_or(OTHER_FAMILY)
    }

    /// Rótulo usado pelo modo de agrupamento pedido.
    pub fn label_for(&self, pkg: &Package, mode: GroupMode) -> String {
        match mode {
            GroupMode::None => String::new(),
            GroupMode::Category => self.category_of(pkg).to_string(),
            GroupMode::Family => self.family_of(pkg).to_string(),
            GroupMode::Repo => pkg.repo_id.clone(),
            GroupMode::Reason => pkg.reason.label().to_string(),
        }
    }

    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
}

/// Um grupo pronto para renderização: rótulo + índices na lista de pacotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: String,
    pub members: Vec<usize>,
}

impl Group {
    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Agrupa a seleção preservando a ordem recebida; grupos ordenados por tamanho.
pub fn group_rows(
    packages: &[Package],
    visible: &[usize],
    mode: GroupMode,
    taxonomy: &Taxonomy,
) -> Vec<Group> {
    if mode == GroupMode::None {
        return vec![Group {
            label: String::new(),
            members: visible.to_vec(),
        }];
    }

    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for idx in visible {
        let Some(pkg) = packages.get(*idx) else {
            continue;
        };
        buckets
            .entry(taxonomy.label_for(pkg, mode))
            .or_default()
            .push(*idx);
    }

    let mut groups: Vec<Group> = buckets
        .into_iter()
        .map(|(label, members)| Group { label, members })
        .collect();
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.label.cmp(&b.label)));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reason;

    fn pkg(name: &str) -> Package {
        Package {
            name: name.into(),
            evr: "1-1.fc45".into(),
            arch: "x86_64".into(),
            repo_id: "@System".into(),
            from_repo_id: None,
            size: 1024,
            install_time: 0,
            summary: String::new(),
            reason: Reason::User,
            nevra: format!("{name}-1-1.fc45.x86_64"),
            is_installed: true,
        }
    }

    #[test]
    fn prefixo_conhecido_vira_categoria_curada() {
        assert_eq!(categorize(&pkg("texlive-a2ping")), "TeX Live");
        assert_eq!(categorize(&pkg("kf6-kcoreaddons")), "KDE Plasma");
        assert_eq!(categorize(&pkg("python3-requests")), "Python");
        assert_eq!(categorize(&pkg("perl-Getopt-Long")), "Perl");
    }

    #[test]
    fn palavra_chave_categoriza_o_que_o_prefixo_nao_pega() {
        assert_eq!(categorize(&pkg("amd-gpu-firmware")), "Firmware");
        assert_eq!(categorize(&pkg("default-fonts-core-sans")), "Fontes");
        assert_eq!(categorize(&pkg("gtk3-devel")), "GNOME");
        assert_eq!(categorize(&pkg("openssl-devel")), "Segurança");
        assert_eq!(categorize(&pkg("libxml2")), "Bibliotecas");
    }

    #[test]
    fn prefixo_desconhecido_cai_em_outros() {
        assert_eq!(categorize(&pkg("zoxide")), "Outros");
    }

    #[test]
    fn familia_agrupa_parentes() {
        assert_eq!(family(&pkg("kf5-kio")), "KDE Plasma");
        assert_eq!(family(&pkg("kf6-kio")), "KDE Plasma");
        assert_eq!(family(&pkg("zoxide")), "Zoxide");
    }

    #[test]
    fn categoria_customizada_vence_a_curada() {
        let packages = vec![pkg("vlc")];
        let mut custom = CustomCategories::empty();
        custom.assign(&packages, "Multimídia pessoal");
        let tax = Taxonomy::build(&packages, &custom);
        assert_eq!(tax.category_of(&packages[0]), "Multimídia pessoal");
        assert!(tax.categories().iter().any(|c| c.custom && c.count == 1));
    }

    #[test]
    fn familias_pequenas_vao_para_outros() {
        let packages = vec![pkg("zoxide"), pkg("bat"), pkg("vlc"), pkg("vlc-plugin")];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        assert_eq!(tax.family_of(&packages[0]), "Outros");
        assert_eq!(tax.family_of(&packages[2]), "Multimídia");
    }

    #[test]
    fn agrupa_por_categoria_ordenando_por_tamanho() {
        let packages = vec![
            pkg("texlive-a"),
            pkg("texlive-b"),
            pkg("vlc"),
            pkg("zoxide"),
        ];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        let visible: Vec<usize> = (0..packages.len()).collect();
        let groups = group_rows(&packages, &visible, GroupMode::Category, &tax);
        assert_eq!(groups[0].label, "TeX Live");
        assert_eq!(groups[0].len(), 2);
        assert!(groups.iter().any(|g| g.label == "Outros"));
    }

    #[test]
    fn sem_grupo_devolve_um_unico_bloco() {
        let packages = vec![pkg("vlc"), pkg("zoxide")];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        let visible = vec![0, 1];
        let groups = group_rows(&packages, &visible, GroupMode::None, &tax);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members, visible);
    }

    #[test]
    fn persistencia_de_categorias() {
        let dir = std::env::temp_dir().join(format!("dnfm-cat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("categories.json");
        let packages = vec![pkg("vlc")];
        let mut custom = CustomCategories::load(&path);
        custom.assign(&packages, "Multimídia");
        custom.save().expect("gravar");

        let reloaded = CustomCategories::load(&path);
        assert_eq!(reloaded.get("vlc", "x86_64"), Some("Multimídia"));
        assert_eq!(reloaded.labels(), vec!["Multimídia".to_string()]);
    }

    #[test]
    fn prune_descarta_pacotes_ausentes() {
        let dir = std::env::temp_dir().join(format!("dnfm-cat2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut custom = CustomCategories::load(dir.join("categories.json"));
        custom.assign(&[pkg("vlc")], "Multimídia");
        custom.assign(&[pkg("firefox")], "Internet");
        assert_eq!(custom.prune(&["vlc"]), 1);
        assert!(custom.get("vlc", "x86_64").is_some());
        assert!(custom.get("firefox", "x86_64").is_none());
    }
    #[test]
    fn appstream_tem_prioridade_sobre_a_regra_curada() {
        let pacotes = vec![pkg("texlive-foo")];
        let mut catalog = crate::metadata::TagCatalog::default();
        catalog.appstream.insert(
            "texlive-foo".into(),
            crate::metadata::AppStreamComponent {
                id: "org.exemplo.Foo.desktop".into(),
                categories: vec!["Graphics".into()],
                summary: None,
            },
        );
        let sem_catalogo = Taxonomy::build(&pacotes, &CustomCategories::empty());
        assert_eq!(sem_catalogo.category_of(&pacotes[0]), "TeX Live");

        let com_catalogo =
            Taxonomy::build_with(&pacotes, &CustomCategories::empty(), Some(&catalog));
        assert_eq!(com_catalogo.category_of(&pacotes[0]), "Gráficos");
    }

    #[test]
    fn familia_vem_do_dominio_da_url() {
        let pacotes = vec![pkg("pacote-sem-prefixo-conhecido")];
        let mut catalog = crate::metadata::TagCatalog::default();
        catalog
            .families
            .insert("pacote-sem-prefixo-conhecido".into(), "Zig".into());
        let tax = Taxonomy::build_with(&pacotes, &CustomCategories::empty(), Some(&catalog));
        assert_eq!(tax.family_of(&pacotes[0]), "Zig");
    }
}
