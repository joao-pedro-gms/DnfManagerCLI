//! Carregamento dos metadados automáticos, com cache em disco.

use std::sync::Arc;

use dnfm_core::{DnfBackend, MetadataCache, TagCatalog};

/// Carrega os metadados automáticos, usando o cache quando a impressão digital
/// dos pacotes instalados não mudou.
pub async fn load_catalog(
    backend: &Arc<dyn DnfBackend>,
    packages: &[dnfm_core::Package],
) -> Result<TagCatalog, dnfm_core::BackendError> {
    let cache = MetadataCache::default();
    let fingerprint = MetadataCache::fingerprint(packages, MetadataCache::catalog_mtime());
    if let Some(catalog) = cache.load(fingerprint) {
        return Ok(catalog);
    }
    let index = backend.file_index().await?;
    let catalog = TagCatalog::build_system(index);
    if let Err(err) = cache.save(&catalog, fingerprint) {
        eprintln!("dnfm: aviso: cache de metadados não gravado: {err}");
    }
    Ok(catalog)
}
