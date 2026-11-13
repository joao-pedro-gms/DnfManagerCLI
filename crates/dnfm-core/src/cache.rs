//! Cache em disco dos metadados derivados (listas de arquivos + AppStream).
//!
//! Obter 825 mil caminhos pelo D-Bus custa ~3,5 s e o catálogo AppStream tem
//! dezenas de MB. Nada disso muda entre transações, então o resultado é gravado
//! em `$XDG_CACHE_HOME/dnfm/metadata.json` e reutilizado enquanto a impressão
//! digital dos pacotes instalados for a mesma.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::TagCatalog;
use crate::model::Package;

/// Muda quando o formato do cache muda (invalida caches antigos).
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    fingerprint: u64,
    catalog: TagCatalog,
}

#[derive(Debug, Clone)]
pub struct MetadataCache {
    path: PathBuf,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self {
            path: Self::default_path(),
        }
    }
}

impl MetadataCache {
    pub fn default_path() -> PathBuf {
        crate::paths::cache_dir().join("metadata.json")
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Impressão digital do conjunto instalado (+ versão do catálogo AppStream).
    pub fn fingerprint(packages: &[Package], catalog_mtime: u64) -> u64 {
        let mut chaves: Vec<&str> = packages.iter().map(|p| p.nevra.as_str()).collect();
        chaves.sort_unstable();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a
        for chave in chaves {
            for byte in chave.as_bytes() {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        for byte in catalog_mtime.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= CACHE_VERSION as u64;
        hash
    }

    /// Carrega apenas se a impressão digital bater.
    pub fn load(&self, fingerprint: u64) -> Option<TagCatalog> {
        let raw = fs::read_to_string(&self.path).ok()?;
        let file: CacheFile = serde_json::from_str(&raw).ok()?;
        if file.version != CACHE_VERSION || file.fingerprint != fingerprint {
            return None;
        }
        Some(file.catalog)
    }

    /// Grava de forma atômica.
    pub fn save(&self, catalog: &TagCatalog, fingerprint: u64) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = CacheFile {
            version: CACHE_VERSION,
            fingerprint,
            catalog: catalog.clone(),
        };
        let body = serde_json::to_vec(&file)?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut fh = fs::File::create(&tmp)?;
            fh.write_all(&body)?;
            fh.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
    }

    /// mtime do catálogo AppStream, para invalidar quando a distro atualiza.
    pub fn catalog_mtime() -> u64 {
        let dir = Path::new("/usr/share/swcatalog/xml");
        let Ok(entries) = fs::read_dir(dir) else {
            return 0;
        };
        entries
            .flatten()
            .filter_map(|e| e.metadata().ok()?.modified().ok())
            .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reason;

    fn pkg(name: &str, evr: &str) -> Package {
        Package {
            name: name.into(),
            evr: evr.into(),
            arch: "x86_64".into(),
            repo_id: "@System".into(),
            from_repo_id: None,
            size: 1,
            install_time: 0,
            summary: String::new(),
            reason: Reason::User,
            nevra: format!("{name}-{evr}.x86_64"),
            is_installed: true,
        }
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dnfm-cache-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("metadata.json")
    }

    #[test]
    fn impressao_digital_muda_com_o_conjunto() {
        let a = vec![pkg("vlc", "1-1")];
        let b = vec![pkg("vlc", "2-1")];
        assert_eq!(
            MetadataCache::fingerprint(&a, 10),
            MetadataCache::fingerprint(&a, 10)
        );
        assert_ne!(
            MetadataCache::fingerprint(&a, 10),
            MetadataCache::fingerprint(&b, 10)
        );
        assert_ne!(
            MetadataCache::fingerprint(&a, 10),
            MetadataCache::fingerprint(&a, 11)
        );
    }

    #[test]
    fn ordem_dos_pacotes_nao_importa() {
        let a = vec![pkg("vlc", "1-1"), pkg("mpv", "1-1")];
        let b = vec![pkg("mpv", "1-1"), pkg("vlc", "1-1")];
        assert_eq!(
            MetadataCache::fingerprint(&a, 5),
            MetadataCache::fingerprint(&b, 5)
        );
    }

    #[test]
    fn grava_e_recarrega_o_catalogo() {
        let cache = MetadataCache::at(temp("roundtrip"));
        let fp = 42;
        assert!(cache.load(fp).is_none(), "ainda não existe");
        let catalog = TagCatalog::default();
        cache.save(&catalog, fp).expect("gravar");
        assert!(cache.load(fp).is_some());
        assert!(cache.load(43).is_none(), "impressão diferente invalida");
    }
}
