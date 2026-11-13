//! Conjunto de pacotes marcados para remoção, persistido em XDG.
//!
//! O arquivo vive em `$XDG_STATE_HOME/dnfm/marks.json` (fallback
//! `~/.local/state/dnfm/marks.json`) e é gravado de forma atômica.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mark {
    pub name: String,
    pub arch: String,
    #[serde(default)]
    pub nevra: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub marked_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarkFile {
    version: u32,
    #[serde(default)]
    marks: Vec<Mark>,
}

/// Marcações ativas, indexadas por `nome.arch`.
#[derive(Debug, Clone, Default)]
pub struct MarkSet {
    path: PathBuf,
    marks: BTreeMap<String, Mark>,
    dirty: bool,
}

fn key(name: &str, arch: &str) -> String {
    format!("{name}.{arch}")
}

impl MarkSet {
    /// Conjunto descartável, sem arquivo (útil em testes e no modo read-only).
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn default_path() -> PathBuf {
        crate::paths::state_dir().join("marks.json")
    }

    /// Carrega as marcações; arquivo ausente ou corrompido vira conjunto vazio.
    pub fn load_default() -> Self {
        Self::load(Self::default_path())
    }

    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut set = Self {
            path,
            marks: BTreeMap::new(),
            dirty: false,
        };
        let raw = match fs::read_to_string(&set.path) {
            Ok(raw) => raw,
            Err(_) => return set,
        };
        match serde_json::from_str::<MarkFile>(&raw) {
            Ok(file) => {
                if file.version > FORMAT_VERSION {
                    tracing::warn!(
                        version = file.version,
                        "marks.json de versão mais nova; ignorando"
                    );
                    return set;
                }
                for mark in file.marks {
                    set.marks.insert(key(&mark.name, &mark.arch), mark);
                }
            }
            Err(err) => {
                // Preserva o arquivo quebrado para diagnóstico em vez de apagar.
                let backup = set.path.with_extension("json.corrupt");
                let _ = fs::rename(&set.path, &backup);
                tracing::warn!(%err, backup = %backup.display(), "marks.json inválido");
            }
        }
        set
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn contains(&self, name: &str, arch: &str) -> bool {
        self.marks.contains_key(&key(name, arch))
    }

    pub fn get(&self, name: &str, arch: &str) -> Option<&Mark> {
        self.marks.get(&key(name, arch))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mark> {
        self.marks.values()
    }

    pub fn names(&self) -> Vec<String> {
        self.marks.values().map(|m| m.name.clone()).collect()
    }

    /// Marca se estiver livre, desmarca se estiver marcado. Devolve o novo estado.
    pub fn toggle(&mut self, name: &str, arch: &str, nevra: &str) -> bool {
        let k = key(name, arch);
        if self.marks.remove(&k).is_some() {
            self.dirty = true;
            false
        } else {
            self.marks.insert(
                k,
                Mark {
                    name: name.to_string(),
                    arch: arch.to_string(),
                    nevra: nevra.to_string(),
                    note: None,
                    marked_at: now_unix(),
                },
            );
            self.dirty = true;
            true
        }
    }

    pub fn mark(&mut self, name: &str, arch: &str, nevra: &str, note: Option<String>) {
        self.marks.insert(
            key(name, arch),
            Mark {
                name: name.to_string(),
                arch: arch.to_string(),
                nevra: nevra.to_string(),
                note,
                marked_at: now_unix(),
            },
        );
        self.dirty = true;
    }

    pub fn unmark(&mut self, name: &str, arch: &str) -> bool {
        let removed = self.marks.remove(&key(name, arch)).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    pub fn clear(&mut self) {
        if !self.marks.is_empty() {
            self.marks.clear();
            self.dirty = true;
        }
    }

    /// Descarta marcações de pacotes que não existem mais no sistema.
    pub fn prune(&mut self, still_installed: &[&str]) -> usize {
        let before = self.marks.len();
        self.marks
            .retain(|k, m| still_installed.iter().any(|n| *n == k || *n == m.name));
        let removed = before - self.marks.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Grava apenas se houve mudança. Escrita atômica: tmp + fsync + rename.
    pub fn save(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = MarkFile {
            version: FORMAT_VERSION,
            marks: self.marks.values().cloned().collect(),
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

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dnfm-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("marks.json")
    }

    #[test]
    fn marca_desmarca_e_conta() {
        let mut set = MarkSet::load(tmp_path("toggle"));
        assert!(set.toggle("vlc", "x86_64", "vlc-1-1.fc45.x86_64"));
        assert!(set.contains("vlc", "x86_64"));
        assert_eq!(set.len(), 1);
        assert!(!set.toggle("vlc", "x86_64", "vlc-1-1.fc45.x86_64"));
        assert!(set.is_empty());
    }

    #[test]
    fn persiste_e_recarrega() {
        let path = tmp_path("persist");
        let mut set = MarkSet::load(&path);
        set.mark("dosbox-staging", "x86_64", "dosbox-1-1.fc45.x86_64", None);
        set.save().expect("gravar");
        assert!(!set.is_dirty());

        let reloaded = MarkSet::load(&path);
        assert!(reloaded.contains("dosbox-staging", "x86_64"));
        assert_eq!(reloaded.len(), 1);
    }

    #[test]
    fn arquivo_corrompido_vira_backup_sem_perder_arquivo() {
        let path = tmp_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ isso nao e json").unwrap();
        let set = MarkSet::load(&path);
        assert!(set.is_empty());
        assert!(path.with_extension("json.corrupt").exists());
    }

    #[test]
    fn prune_remove_pacotes_que_sumirem() {
        let mut set = MarkSet::load(tmp_path("prune"));
        set.mark("vlc", "x86_64", "vlc-1-1.fc45.x86_64", None);
        set.mark("firefox", "x86_64", "firefox-1-1.fc45.x86_64", None);
        let removed = set.prune(&["vlc"]);
        assert_eq!(removed, 1);
        assert!(set.contains("vlc", "x86_64"));
        assert!(!set.contains("firefox", "x86_64"));
    }

    #[test]
    fn arquivo_de_versao_futura_e_ignorado() {
        let path = tmp_path("future");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"version": 99, "marks": []}"#).unwrap();
        let set = MarkSet::load(&path);
        assert!(set.is_empty());
    }
}
