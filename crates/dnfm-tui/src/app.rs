//! Estado da aplicação e transições. Nada aqui desenha nem fala com o D-Bus:
//! efeitos são devolvidos como [`Cmd`] para o laço de eventos executar.

mod filters;
mod state;
mod types;

pub use state::*;
pub use types::*;

use std::collections::HashSet;

use dnfm_core::{
    categories::{group_rows, CustomCategories, GroupMode, Taxonomy},
    Filter, HistoryEntry, MarkSet, Package, PackageDetails, ProgressEvent, Query, Repo, SortKey,
    Upgrade,
};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;

pub struct App {
    pub tab: Tab,
    pub installed: Vec<Package>,
    /// Índices (em `installed`) que passaram pelo filtro, na ordem escolhida.
    pub visible: Vec<usize>,
    /// Linhas exibidas: cabeçalhos de grupo intercalados com pacotes.
    pub rows: Vec<RowRef>,
    pub table: TableState,
    pub scroll: usize,
    pub viewport: usize,
    pub table_area: Rect,
    pub marks: MarkSet,
    pub custom_categories: CustomCategories,
    pub taxonomy: Taxonomy,
    pub group: GroupMode,
    pub collapsed: HashSet<String>,
    pub filter: Filter,
    pub filter_panel: Option<FilterPanel>,
    pub picker: Option<Picker>,
    pub assign_input: Option<String>,
    /// `Some(buffer)` enquanto o usuário digita a busca.
    pub search: Option<String>,
    pub sort: SortKey,
    pub descending: bool,
    pub upgrades: Vec<Upgrade>,
    pub upgradable_names: HashSet<String>,
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
    pub pending_specs: Vec<String>,
    pub last_refresh: Option<std::time::Instant>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::Installed,
            installed: Vec::new(),
            visible: Vec::new(),
            rows: Vec::new(),
            table: TableState::default(),
            scroll: 0,
            viewport: 20,
            table_area: Rect::default(),
            marks: MarkSet::empty(),
            custom_categories: CustomCategories::empty(),
            taxonomy: Taxonomy::default(),
            group: GroupMode::None,
            collapsed: HashSet::new(),
            filter: Filter::default(),
            filter_panel: None,
            picker: None,
            assign_input: None,
            search: None,
            sort: SortKey::Name,
            descending: false,
            upgrades: Vec::new(),
            upgradable_names: HashSet::new(),
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
            pending_specs: Vec::new(),
            last_refresh: None,
        }
    }
}

impl App {
    pub fn new(marks: MarkSet, custom: CustomCategories) -> Self {
        Self {
            marks,
            custom_categories: custom,
            ..Default::default()
        }
    }

    pub fn rows_len(&self) -> usize {
        match self.tab {
            Tab::Installed => self.rows.len(),
            Tab::Updates => self.upgrades.len(),
            Tab::Orphans => self.orphans.len(),
            Tab::History => self.history.len(),
            Tab::Repos => self.repos.len(),
        }
    }

    pub fn selected_index(&self) -> usize {
        let len = self.rows_len();
        if len == 0 {
            return 0;
        }
        self.table.selected().unwrap_or(0).min(len - 1)
    }

    pub fn current_row(&self) -> Option<&RowRef> {
        if self.tab != Tab::Installed {
            return None;
        }
        self.rows.get(self.selected_index())
    }

    pub fn current_package(&self) -> Option<&Package> {
        let idx = self.selected_index();
        match self.tab {
            Tab::Installed => match self.rows.get(idx)? {
                RowRef::Package { index } => self.installed.get(*index),
                RowRef::Header { .. } => None,
            },
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

    pub fn category_label(&self, pkg: &Package) -> &str {
        self.taxonomy.category_of(pkg)
    }

    /// Recalcula filtro, ordenação, grupos e a lista de linhas.
    pub fn refresh(&mut self) {
        self.upgradable_names = self
            .upgrades
            .iter()
            .map(|u| u.installed.name.clone())
            .collect();

        self.visible = Query {
            filter: &self.filter,
            sort: self.sort,
            descending: self.descending,
            marks: &self.marks,
            taxonomy: &self.taxonomy,
            upgradable: &self.upgradable_names,
        }
        .apply(&self.installed);

        self.rebuild_rows();

        if !self.installed.is_empty() {
            let names: Vec<&str> = self.installed.iter().map(|p| p.name.as_str()).collect();
            self.marks.prune(&names);
            self.custom_categories.prune(&names);
        }

        self.ensure_package_selection();
        self.last_refresh = Some(std::time::Instant::now());
    }

    /// Reconstrói as linhas a partir dos grupos, respeitando os recolhidos.
    pub fn rebuild_rows(&mut self) {
        let groups = group_rows(&self.installed, &self.visible, self.group, &self.taxonomy);
        self.rows.clear();
        if self.group == GroupMode::None {
            for idx in &self.visible {
                self.rows.push(RowRef::Package { index: *idx });
            }
            return;
        }
        for group in groups {
            let collapsed = self.collapsed.contains(&group.label);
            self.rows.push(RowRef::Header {
                label: group.label.clone(),
                count: group.len(),
                collapsed,
            });
            if collapsed {
                continue;
            }
            for idx in group.members {
                self.rows.push(RowRef::Package { index: idx });
            }
        }
    }

    /// Garante que o cursor esteja sobre um pacote (nunca sobre cabeçalho).
    pub fn ensure_package_selection(&mut self) {
        if self.tab != Tab::Installed {
            let len = self.rows_len();
            if len == 0 {
                self.table.select(None);
            } else if self.table.selected().unwrap_or(0) >= len {
                self.table.select(Some(len - 1));
            }
            return;
        }
        if self.rows.is_empty() {
            self.table.select(None);
            return;
        }
        let mut idx = self.table.selected().unwrap_or(0);
        if idx >= self.rows.len() {
            idx = self.rows.len() - 1;
        }
        if self.rows[idx].is_header() {
            match self.rows[idx..]
                .iter()
                .position(|row| !row.is_header())
                .map(|offset| idx + offset)
            {
                Some(next) => idx = next,
                None => {
                    // Grupo recolhido no fim da lista: volta para o primeiro pacote.
                    if let Some(first) = self.rows.iter().position(|row| !row.is_header()) {
                        idx = first;
                    }
                }
            }
        }
        self.table.select(Some(idx));
        self.follow_selection();
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.rows_len();
        if len == 0 {
            self.table.select(None);
            return;
        }
        if self.tab != Tab::Installed {
            let next = (self.selected_index() as isize + delta).clamp(0, len as isize - 1);
            self.table.select(Some(next as usize));
            self.follow_selection();
            return;
        }

        let step: isize = if delta >= 0 { 1 } else { -1 };
        let mut remaining = delta.abs();
        let mut idx = self.selected_index() as isize;
        while remaining > 0 {
            let mut next = idx + step;
            while next >= 0 && (next as usize) < len && self.rows[next as usize].is_header() {
                next += step;
            }
            if next < 0 || next as usize >= len {
                break;
            }
            idx = next;
            remaining -= 1;
        }
        self.table.select(Some(idx as usize));
        self.follow_selection();
    }

    pub fn select(&mut self, index: usize) {
        if self.rows_len() == 0 {
            return;
        }
        let index = index.min(self.rows_len() - 1);
        self.table.select(Some(index));
        self.ensure_package_selection();
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

    /// Troca a aba e reposiciona o cursor.
    pub fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.scroll = 0;
        self.table.select(Some(0));
        self.ensure_package_selection();
    }

    /// `espaço`: em cabeçalho recolhe/expande o grupo; em pacote marca.
    pub fn toggle_current(&mut self) {
        if let Some(RowRef::Header { label, .. }) = self.current_row().cloned() {
            self.toggle_group(&label);
            return;
        }
        self.toggle_mark_current();
    }

    pub fn toggle_group(&mut self, label: &str) {
        if !self.collapsed.remove(label) {
            self.collapsed.insert(label.to_string());
        }
        self.rebuild_rows();
        self.ensure_package_selection();
        let collapsed = self.collapsed.contains(label);
        self.status = format!(
            "grupo {label} {}",
            if collapsed { "recolhido" } else { "expandido" }
        );
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
    }

    /// Marca todos os pacotes do grupo atual (ou todos os visíveis sem grupo).
    pub fn mark_all_visible(&mut self) {
        let targets: Vec<usize> = match self.current_row().cloned() {
            Some(RowRef::Header { label, .. }) if self.group != GroupMode::None => self
                .visible
                .iter()
                .copied()
                .filter(|idx| {
                    self.installed
                        .get(*idx)
                        .map(|p| self.taxonomy.label_for(p, self.group) == label)
                        .unwrap_or(false)
                })
                .collect(),
            _ => self.visible.clone(),
        };
        let count = targets.len();
        for idx in targets {
            if let Some(pkg) = self.installed.get(idx).cloned() {
                if dnfm_core::safety::is_protected(&pkg.name).is_none()
                    && !self.marks.contains(&pkg.name, &pkg.arch)
                {
                    self.marks.mark(&pkg.name, &pkg.arch, &pkg.nevra, None);
                }
            }
        }
        self.status = format!("{count} pacotes marcados");
    }

    pub fn clear_marks(&mut self) {
        let count = self.marks.len();
        self.marks.clear();
        self.status = format!("{count} marcações removidas");
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

    /// Pedido de preview: marcações (se houver) ou o pacote sob o cursor.
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
                self.taxonomy = Taxonomy::build(&self.installed, &self.custom_categories);
                self.refresh();
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
                    return Some(Cmd::LoadDetails(nevra));
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
                if matches!(self.modal, Some(Modal::Progress)) {
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
