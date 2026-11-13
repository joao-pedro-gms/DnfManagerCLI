//! Estado da aplicação e transições. Nada aqui desenha nem fala com o D-Bus:
//! efeitos são devolvidos como [`Cmd`] para o laço de eventos executar.

mod types;

use std::collections::HashSet;

use dnfm_core::{
    select, Filter, HistoryEntry, MarkSet, Package, PackageDetails, ProgressEvent, Repo, SortKey,
    Upgrade,
};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;

pub use types::*;

pub struct App {
    pub tab: Tab,
    pub installed: Vec<Package>,
    pub visible: Vec<usize>,
    pub table: TableState,
    pub scroll: usize,
    pub viewport: usize,
    pub table_area: Rect,
    pub marks: MarkSet,
    pub filter: Filter,
    pub sort: SortKey,
    pub descending: bool,
    /// `Some(buffer)` enquanto o usuário digita a busca.
    pub search: Option<String>,
    pub upgrades: Vec<Upgrade>,
    pub selected_upgrades: HashSet<String>,
    pub orphans: Vec<Package>,
    pub history: Vec<HistoryEntry>,
    pub repos: Vec<Repo>,
    pub details: Option<PackageDetails>,
    pub modal: Option<Modal>,
    pub progress: Progress,
    pub status: String,
    pub loading: bool,
    pub quit: bool,
    pub upgrades_loaded: bool,
    pub orphans_loaded: bool,
    pub orphans_loading: bool,
    pub history_loaded: bool,
    pub history_loading: bool,
    pub last_refresh: Option<std::time::Instant>,
    pub pending_specs: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::Installed,
            installed: Vec::new(),
            visible: Vec::new(),
            table: TableState::default(),
            scroll: 0,
            viewport: 20,
            table_area: Rect::default(),
            marks: MarkSet::empty(),
            filter: Filter::default(),
            sort: SortKey::Name,
            descending: false,
            search: None,
            upgrades: Vec::new(),
            selected_upgrades: HashSet::new(),
            orphans: Vec::new(),
            history: Vec::new(),
            repos: Vec::new(),
            details: None,
            modal: None,
            progress: Progress::default(),
            status: "carregando…".to_string(),
            loading: true,
            quit: false,
            upgrades_loaded: false,
            orphans_loaded: false,
            orphans_loading: false,
            history_loaded: false,
            history_loading: false,
            last_refresh: None,
            pending_specs: Vec::new(),
        }
    }
}

impl App {
    pub fn with_marks(marks: MarkSet) -> Self {
        Self {
            marks,
            ..Default::default()
        }
    }

    pub fn rows_len(&self) -> usize {
        match self.tab {
            Tab::Installed => self.visible.len(),
            Tab::Updates => self.upgrades.len(),
            Tab::Orphans => self.orphans.len(),
            Tab::History => self.history.len(),
            Tab::Repos => self.repos.len(),
        }
    }

    pub fn selected_index(&self) -> usize {
        self.table
            .selected()
            .unwrap_or(0)
            .min(self.rows_len().saturating_sub(1))
    }

    pub fn current_package(&self) -> Option<&Package> {
        let idx = self.selected_index();
        match self.tab {
            Tab::Installed => self.visible.get(idx).and_then(|i| self.installed.get(*i)),
            Tab::Orphans => self.orphans.get(idx),
            _ => None,
        }
    }

    pub fn current_upgrade(&self) -> Option<&Upgrade> {
        self.upgrades.get(self.selected_index())
    }

    pub fn is_marked(&self, pkg: &Package) -> bool {
        self.marks.contains(&pkg.name, &pkg.arch)
    }

    /// Recalcula a lista visível aplicando filtro, busca e ordenação.
    pub fn refresh(&mut self) {
        self.filter.marked_only = false;
        self.visible = select(
            &self.installed,
            &self.filter,
            self.sort,
            self.descending,
            &self.marks,
        );
        self.last_refresh = Some(std::time::Instant::now());
        let max = self.visible.len().saturating_sub(1);
        if let Some(sel) = self.table.selected() {
            self.table.select(Some(sel.min(max)));
        } else if !self.visible.is_empty() {
            self.table.select(Some(0));
        }
        // Descarta marcações de pacotes que não existem mais.
        if !self.installed.is_empty() {
            let names: Vec<&str> = self.installed.iter().map(|p| p.name.as_str()).collect();
            self.marks.prune(&names);
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.rows_len();
        if len == 0 {
            self.table.select(None);
            return;
        }
        let current = self.selected_index() as isize;
        let next = (current + delta).clamp(0, len as isize - 1);
        self.table.select(Some(next as usize));
        self.follow_selection();
    }

    pub fn select(&mut self, index: usize) {
        if self.rows_len() == 0 {
            return;
        }
        let index = index.min(self.rows_len() - 1);
        self.table.select(Some(index));
        self.follow_selection();
    }

    /// Mantém a linha selecionada dentro da janela visível.
    pub fn follow_selection(&mut self) {
        let sel = self.selected_index();
        if sel < self.scroll {
            self.scroll = sel;
        } else if sel >= self.scroll + self.viewport.max(1) {
            self.scroll = sel + 1 - self.viewport.max(1);
        }
    }

    pub fn toggle_mark_current(&mut self) {
        let Some(pkg) = self.current_package().cloned() else {
            return;
        };
        if let Some(reason) = dnfm_core::safety::protected_reason(&pkg.name, None) {
            self.status = format!("{} é protegido ({reason})", pkg.name);
            return;
        }
        let marked = self.marks.toggle(&pkg.name, &pkg.arch, &pkg.nevra);
        self.status = if marked {
            format!("{} marcado para remoção", pkg.name)
        } else {
            format!("{} desmarcado", pkg.name)
        };
        self.refresh();
    }

    pub fn mark_all_visible(&mut self) {
        let packages: Vec<Package> = self
            .visible
            .iter()
            .filter_map(|i| self.installed.get(*i))
            .filter(|p| dnfm_core::safety::is_protected(&p.name).is_none())
            .cloned()
            .collect();
        let count = packages.len();
        for pkg in packages {
            if !self.marks.contains(&pkg.name, &pkg.arch) {
                self.marks.mark(&pkg.name, &pkg.arch, &pkg.nevra, None);
            }
        }
        self.status = format!("{count} pacotes marcados");
        self.refresh();
    }

    pub fn clear_marks(&mut self) {
        let count = self.marks.len();
        self.marks.clear();
        self.status = format!("{count} marcações removidas");
        self.refresh();
    }

    pub fn toggle_upgrade_selection(&mut self) {
        let Some(up) = self.current_upgrade().cloned() else {
            return;
        };
        if !self.selected_upgrades.remove(&up.installed.name) {
            self.selected_upgrades.insert(up.installed.name.clone());
        }
    }

    pub fn select_all_upgrades(&mut self, security_only: bool) {
        for up in &self.upgrades {
            if !security_only || up.severity.is_security_relevant() {
                self.selected_upgrades.insert(up.installed.name.clone());
            }
        }
        self.status = format!("{} atualizações selecionadas", self.selected_upgrades.len());
    }

    /// Pedido de preview: marcações (se houver) ou o pacote sob o cursor.
    /// Pede a carga da aba atual se ela ainda não foi visitada.
    pub fn lazy_request(&mut self) -> Option<Cmd> {
        match self.tab {
            Tab::Orphans if !self.orphans_loaded && !self.orphans_loading => {
                self.orphans_loading = true;
                Some(Cmd::LoadOrphans)
            }
            Tab::History if !self.history_loaded && !self.history_loading => {
                self.history_loading = true;
                Some(Cmd::LoadHistory)
            }
            _ => None,
        }
    }

    pub fn request_preview(&mut self) -> Option<Cmd> {
        let specs: Vec<String> = if !self.marks.is_empty() {
            self.marks.names()
        } else {
            self.current_package()
                .map(|p| vec![p.name.clone()])
                .unwrap_or_default()
        };
        if specs.is_empty() {
            self.status = "nada selecionado para remover".to_string();
            return None;
        }
        self.loading = true;
        self.pending_specs = specs.clone();
        self.status = format!("resolvendo remoção de {} pacote(s)…", specs.len());
        Some(Cmd::Preview(specs))
    }

    pub fn apply_event(&mut self, event: AppEvent) -> Option<Cmd> {
        match event {
            AppEvent::Installed(packages) => {
                self.installed = packages;
                self.loading = false;
                self.refresh();
                self.status = format!(
                    "{} instalados · {} marcados",
                    self.installed.len(),
                    self.marks.len()
                );
                let name = self
                    .current_package()
                    .map(|p| p.nevra.clone())
                    .unwrap_or_default();
                if !name.is_empty() {
                    return Some(Cmd::LoadDetails(name));
                }
                None
            }
            AppEvent::Upgrades(list) => {
                self.upgrades = list;
                self.upgrades_loaded = true;
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
                self.modal = Some(Modal::Preview(*preview));
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
                    self.modal = None;
                    return Some(Cmd::LoadAll);
                }
                self.status = "a transação falhou".to_string();
                self.modal = Some(Modal::Error("a transação não foi concluída".to_string()));
                None
            }
            AppEvent::Progress(event) => {
                self.apply_progress(event);
                None
            }
            AppEvent::Failed(message) => {
                self.loading = false;
                if self.modal_matches_progress() {
                    self.progress.success = Some(false);
                }
                self.status = message.clone();
                self.modal = Some(Modal::Error(message));
                None
            }
        }
    }

    fn preview_names(&self) -> Vec<(String, String)> {
        match &self.modal {
            Some(Modal::Preview(preview)) => preview
                .removals()
                .map(|i| (i.package.name.clone(), i.package.arch.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn modal_matches_progress(&self) -> bool {
        matches!(self.modal, Some(Modal::Progress))
    }

    pub fn apply_progress(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Begin { total } => {
                self.progress = Progress {
                    total,
                    ..Default::default()
                };
                self.modal = Some(Modal::Progress);
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
                self.modal = Some(Modal::Progress);
                self.progress = Progress::default();
                self.status = "executando transação…".to_string();
                Some(Cmd::Execute(token))
            }
            Err(err) => {
                self.modal = Some(Modal::Error(err.to_string()));
                None
            }
        }
    }

    pub fn cancel_modal(&mut self) -> Option<Cmd> {
        let had_preview = matches!(self.modal, Some(Modal::Preview(_)));
        self.modal = None;
        if had_preview {
            self.status = "preview descartado".to_string();
            return Some(Cmd::CancelPreview);
        }
        None
    }

    /// Nome curto para a barra inferior: `nome-versão` sem arch/epoch.
    pub fn selection_label(&self) -> String {
        match self.tab {
            Tab::Updates => self
                .current_upgrade()
                .map(|u| format!("{} → {}", u.name(), u.candidate.evr))
                .unwrap_or_default(),
            Tab::History => self
                .history
                .get(self.selected_index())
                .map(|e| format!("#{} {}", e.id, e.description))
                .unwrap_or_default(),
            Tab::Repos => self
                .repos
                .get(self.selected_index())
                .map(|r| r.id.clone())
                .unwrap_or_default(),
            _ => self
                .current_package()
                .map(|p| p.nevra.clone())
                .unwrap_or_default(),
        }
    }
}

pub fn short_nevra(nevra: &str) -> String {
    let base = nevra.rsplit(':').next().unwrap_or(nevra);
    if base.chars().count() <= 44 {
        base.to_string()
    } else {
        let cut: String = base.chars().take(41).collect();
        format!("{cut}…")
    }
}
