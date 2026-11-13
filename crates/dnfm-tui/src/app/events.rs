//! Eventos do backend e progresso de transação aplicados ao estado.

use super::*;

impl App {
    pub fn apply_event(&mut self, event: AppEvent) -> Option<Cmd> {
        match event {
            AppEvent::Installed(packages) => {
                self.installed = packages;
                self.loading = false;
                self.taxonomy = if self.catalog.is_empty() {
                    Taxonomy::build(&self.installed, &self.custom_categories)
                } else {
                    Taxonomy::build_with(
                        &self.installed,
                        &self.custom_categories,
                        Some(&self.catalog),
                    )
                };
                self.refresh();
                self.anim.start_bars();
                self.status = format!(
                    "{} instalados · {} categorias · {} marcados",
                    self.installed.len(),
                    self.taxonomy.category_count(),
                    self.marks.len()
                );
                let nevra = self
                    .current_package()
                    .map(|p| p.nevra.clone())
                    .unwrap_or_default();
                if !nevra.is_empty() {
                    self.extra_cmds.push(Cmd::LoadDetails(nevra));
                }
                // Metadados automáticos em segundo plano (usa cache quando bate).
                Some(Cmd::LoadMetadata(self.metadata_fingerprint()))
            }
            AppEvent::Metadata(catalog) => {
                self.catalog = *catalog;
                self.catalog_loaded = true;
                self.rebuild_taxonomy();
                self.stats_dirty = true;
                let com_categoria = self
                    .catalog
                    .appstream
                    .keys()
                    .filter(|n| self.installed.iter().any(|p| &p.name == *n))
                    .count();
                self.status = format!(
                    "metadados: {} categorias · {} com AppStream · {} com arquivos",
                    self.taxonomy.category_count(),
                    com_categoria,
                    self.catalog.files.len().min(self.installed.len())
                );
                if self.tab == Tab::Stats {
                    self.ensure_stats();
                }
                None
            }
            AppEvent::Upgrades(list) => {
                self.upgrades = list;
                self.upgrades_loaded = true;
                self.refresh();
                None
            }
            AppEvent::Orphans(list) => {
                self.orphans = list;
                self.orphans_loaded = true;
                self.orphans_loading = false;
                None
            }
            AppEvent::History(list) => {
                self.history = list;
                self.history_loaded = true;
                self.history_loading = false;
                None
            }
            AppEvent::Repos(list) => {
                self.repos = list;
                None
            }
            AppEvent::Details(details) => {
                if let Some(inspection) = self.inspection.as_mut() {
                    inspection.details = Some(details.clone());
                }
                self.details = Some(details);
                None
            }
            AppEvent::Preview(preview) => {
                self.loading = false;
                if preview.is_failed() {
                    self.status = "resolução falhou — veja os problemas".to_string();
                } else {
                    self.status = format!(
                        "{} removidos · {} adicionados · {} liberados",
                        preview.removals().count(),
                        preview.additions().count(),
                        dnfm_core::format_size(preview.freed_bytes())
                    );
                }
                self.open_modal(Modal::Preview(*preview));
                None
            }
            AppEvent::Executed(outcome) => {
                self.loading = false;
                if outcome.success {
                    let applied = self.preview_names();
                    for (name, arch) in applied {
                        self.marks.unmark(&name, &arch);
                    }
                    let _ = self.marks.save();
                    self.status = "transação concluída".to_string();
                    self.close_modal();
                    return Some(Cmd::LoadAll);
                }
                self.status = "a transação falhou".to_string();
                self.open_modal(Modal::Error("a transação não foi concluída".to_string()));
                None
            }
            AppEvent::Progress(event) => {
                self.apply_progress(event);
                None
            }
            AppEvent::Failed(message) => {
                self.loading = false;
                if matches!(self.modal, Some(Modal::Progress)) {
                    self.progress.success = Some(false);
                }
                self.status = message.clone();
                self.open_modal(Modal::Error(message));
                None
            }
        }
    }

    pub(crate) fn preview_names(&self) -> Vec<(String, String)> {
        match &self.modal {
            Some(Modal::Preview(preview)) => preview
                .removals()
                .map(|i| (i.package.name.clone(), i.package.arch.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn apply_progress(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Begin { total } => {
                self.progress = Progress {
                    total,
                    ..Default::default()
                };
                self.open_modal(Modal::Progress);
            }
            ProgressEvent::Download {
                description,
                downloaded,
                total,
            } => {
                self.progress.total = total;
                self.progress.done = downloaded;
                self.progress.current = description;
            }
            ProgressEvent::ActionStart { nevra, action, .. } => {
                self.progress.current = format!("{} {}", action.label(), short_nevra(&nevra));
                self.progress
                    .log
                    .push(format!("{} {}", action.label(), short_nevra(&nevra)));
                if self.progress.log.len() > 200 {
                    self.progress.log.remove(0);
                }
            }
            ProgressEvent::ActionProgress {
                processed, total, ..
            } => {
                self.progress.done = processed;
                self.progress.total = total;
            }
            ProgressEvent::ActionStop { .. } => {}
            ProgressEvent::ScriptError { nevra, code } => {
                self.progress.log.push(format!(
                    "scriptlet de {} retornou {code}",
                    short_nevra(&nevra)
                ));
            }
            ProgressEvent::Done { success } => {
                self.progress.success = Some(success);
                self.progress.done = self.progress.total;
            }
        }
    }

    pub fn confirm_preview(&mut self) -> Option<Cmd> {
        let Some(Modal::Preview(preview)) = &self.modal else {
            return None;
        };
        match preview.clone().confirm() {
            Ok(token) => {
                self.open_modal(Modal::Progress);
                self.progress = Progress::default();
                self.status = "executando transação…".to_string();
                Some(Cmd::Execute(token))
            }
            Err(err) => {
                self.open_modal(Modal::Error(err.to_string()));
                None
            }
        }
    }
}
