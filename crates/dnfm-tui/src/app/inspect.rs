//! Estatísticas, metadados e inspetor de pacote.

use super::*;

impl App {
    /// Recalcula as estatísticas se algo mudou desde a última visita.
    pub fn ensure_stats(&mut self) {
        if !self.stats_dirty && self.stats.total == self.installed.len() {
            return;
        }
        self.stats = dnfm_core::Stats::build(
            &self.installed,
            &self.upgrades,
            &self.marks,
            &self.taxonomy,
            &self.catalog,
        );
        self.stats_dirty = false;
    }

    /// Impressão digital para o cache de metadados.
    pub fn metadata_fingerprint(&self) -> u64 {
        dnfm_core::MetadataCache::fingerprint(
            &self.installed,
            dnfm_core::MetadataCache::catalog_mtime(),
        )
    }

    /// Abre o inspetor do pacote sob o cursor e pede os detalhes que faltam.
    pub fn open_inspector(&mut self) -> Option<Cmd> {
        let pkg = self.current_package().cloned()?;
        self.inspection = Some(Inspection {
            categories: self.catalog.category_for(&pkg).into_iter().collect(),
            tags: self.catalog.tags_for(&pkg),
            files: self.catalog.file_info(&pkg.name).cloned(),
            details: self.details.clone(),
            package: pkg.clone(),
        });
        self.open_modal(Modal::Inspector);
        Some(Cmd::LoadDetails(pkg.nevra))
    }
    /// Nome da origem da categoria, para exibir na interface.
    pub fn metadata_state(&self) -> &'static str {
        if self.catalog.is_empty() {
            "sem metadados"
        } else if self.catalog_loaded {
            "metadados prontos"
        } else {
            "carregando metadados"
        }
    }
}
