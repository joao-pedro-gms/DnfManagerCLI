//! Estado da aplicação e transições. Nada aqui desenha nem fala com o D-Bus:
//! efeitos são devolvidos como [`Cmd`] para o laço de eventos executar.

mod events;
mod filters;
mod inspect;
mod mark_ops;
mod nav;
mod sidebar;
mod state;
mod types;

pub use sidebar::{Bucket, Sidebar};
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

use crate::anim::Animator;
use crate::theme::{Theme, UiSettings};

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
    pub anim: Animator,
    pub theme: Theme,
    pub ui: UiSettings,
    pub sidebar: Sidebar,
    /// Linha sob o mouse, para realce de hover.
    pub hover: Option<usize>,
    /// Área e rolagem do sidebar, preenchidas no desenho para hit-testing.
    pub sidebar_area: Rect,
    pub sidebar_scroll: usize,
    pub catalog: dnfm_core::TagCatalog,
    pub catalog_loaded: bool,
    pub stats: dnfm_core::Stats,
    pub stats_dirty: bool,
    pub inspection: Option<Inspection>,
    /// Comandos extras gerados por um evento (drenados pelo laço).
    pub extra_cmds: Vec<Cmd>,
    /// Chips de filtro desenhados, com a área para clique.
    pub chips: Vec<(Rect, ChipClear)>,
    /// Áreas das abas, para clique preciso.
    pub tab_hits: Vec<(Rect, Tab)>,
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
            anim: Animator::default(),
            theme: Theme::default(),
            ui: UiSettings::default(),
            sidebar: Sidebar::default(),
            hover: None,
            sidebar_area: Rect::default(),
            sidebar_scroll: 0,
            catalog: dnfm_core::TagCatalog::default(),
            catalog_loaded: false,
            stats: dnfm_core::Stats::default(),
            stats_dirty: true,
            inspection: None,
            extra_cmds: Vec::new(),
            chips: Vec::new(),
            tab_hits: Vec::new(),
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
            Tab::Stats => 0,
        }
    }

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

        self.stats_dirty = true;
        self.ensure_package_selection();
        self.rebuild_sidebar();
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
    pub fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.scroll = 0;
        self.table.select(Some(0));
        self.ensure_package_selection();
        if tab == Tab::Stats {
            self.ensure_stats();
        }
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

    pub fn open_modal(&mut self, modal: Modal) {
        let already_open = self.modal.is_some() && !self.anim.closing;
        self.modal = Some(modal);
        if !already_open {
            self.anim.open_modal();
            self.anim.pulse();
        }
    }

    /// Fecha com transição; o modal só desaparece quando a animação termina.
    pub fn close_modal(&mut self) {
        if self.modal.is_some() && !self.anim.closing {
            self.anim.close_modal();
            self.anim.pulse();
        }
    }

    pub fn cancel_modal(&mut self) -> Option<Cmd> {
        let had_preview = matches!(self.modal, Some(Modal::Preview(_)));
        self.close_modal();
        if had_preview {
            self.status = "preview descartado".to_string();
            return Some(Cmd::CancelPreview);
        }
        None
    }

    /// Avança a animação; devolve `true` se a tela precisa ser redesenhada.
    pub fn on_tick(&mut self) -> bool {
        let animating = self.anim.tick();
        if self.anim.closing && self.anim.modal <= 0.0 {
            self.modal = None;
            self.anim.closing = false;
        }
        animating
    }

    /// Leva as animações ao fim (usado nos testes e ao trocar de tela).
    pub fn settle(&mut self) {
        for _ in 0..60 {
            self.anim.advance(0.05);
            if !self.anim.is_animating() {
                break;
            }
        }
        if self.anim.closing && self.anim.modal <= 0.0 {
            self.modal = None;
            self.anim.closing = false;
        }
    }

    /// Remove um filtro específico (clique no chip correspondente).
    pub fn clear_chip(&mut self, chip: &ChipClear) {
        match chip {
            ChipClear::Text => self.filter.text.clear(),
            ChipClear::Category(label) => self.filter.categories.retain(|c| c != label),
            ChipClear::Family(label) => self.filter.families.retain(|f| f != label),
            ChipClear::Repo => self.filter.repo = None,
            ChipClear::Reason => self.filter.reason = None,
            ChipClear::Arch => self.filter.arch = None,
            ChipClear::Size => self.filter.apply_size_preset(dnfm_core::SizePreset::Any),
            ChipClear::Date => self.filter.apply_date_preset(dnfm_core::DatePreset::Any, 0),
            ChipClear::Marked => self.filter.marked_only = false,
            ChipClear::Protected => self.filter.protected = None,
            ChipClear::Updates => self.filter.only_updates = false,
        }
        self.after_filter_change();
        self.status = "filtro removido".to_string();
        self.anim.pulse();
    }

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
        self.ui.dirty = true;
        self.anim.pulse();
        self.status = format!("tema {}", self.theme.name);
    }

    /// Um pulso de destaque (usado quando o conteúdo muda).
    pub fn flash(&mut self) {
        self.anim.pulse();
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
