//! Categorias e famílias de pacotes.
//!
//! No Fedora atual o tag `Group` do RPM está morto (quase tudo "Unspecified") e
//! o comps cobre uma fração pequena da base instalada. A taxonomia aqui é:
//!
//! 1. **categoria própria** — o que o usuário atribuiu, persistido em XDG;
//! 2. **categoria curada** — regras por prefixo/palavra-chave (grossa);
//! 3. **família** — prefixo do nome, com apelidos que juntam parentes
//!    (`kf5`, `kf6`, `plasma` viram "KDE Plasma").

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::model::Package;
use serde::{Deserialize, Serialize};

mod rules;
mod taxonomy;

pub use rules::{categorize, family};
pub use taxonomy::{group_rows, CategoryInfo, Group, Taxonomy};

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

fn package_key(name: &str, arch: &str) -> String {
    format!("{name}.{arch}")
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
