//! Sidebar de grupos: a mesma taxonomia do agrupamento, mas sempre visível,
//! com contagem por grupo e seleção que vira filtro.
//!
//! Serve para responder "onde estão os pacotes?" sem trocar de aba nem abrir
//! modal: `[`/`]` percorrem os grupos aplicando o filtro na hora.

use dnfm_core::categories::GroupMode;
use dnfm_core::Reason;

use super::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub label: String,
    pub count: usize,
    pub selected: bool,
}

impl Bucket {
    pub fn share(&self, max: usize) -> f32 {
        if max == 0 {
            0.0
        } else {
            self.count as f32 / max as f32
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sidebar {
    pub visible: bool,
    pub title: String,
    pub mode: GroupMode,
    pub buckets: Vec<Bucket>,
    pub cursor: usize,
    pub total: usize,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            visible: true,
            title: "Categorias".to_string(),
            mode: GroupMode::Category,
            buckets: Vec::new(),
            cursor: 0,
            total: 0,
        }
    }
}

impl Sidebar {
    pub fn max_count(&self) -> usize {
        self.buckets.iter().map(|b| b.count).max().unwrap_or(0)
    }

    pub fn current(&self) -> Option<&Bucket> {
        self.buckets.get(self.cursor)
    }

    /// Quantos grupos selecionados (para o rodapé do painel).
    pub fn selected_count(&self) -> usize {
        self.buckets.iter().filter(|b| b.selected).count()
    }
}

impl App {
    /// Recalcula os grupos do sidebar a partir do estado atual.
    pub fn rebuild_sidebar(&mut self) {
        let mode = match self.group {
            GroupMode::None => GroupMode::Category,
            other => other,
        };
        self.sidebar.mode = mode;
        self.sidebar.title = match mode {
            GroupMode::Category => "Categorias",
            GroupMode::Family => "Famílias",
            GroupMode::Repo => "Repositórios",
            GroupMode::Reason => "Motivo",
            GroupMode::None => "Categorias",
        }
        .to_string();

        let mut counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for pkg in &self.installed {
            let label = match mode {
                GroupMode::Repo => pkg.repo_id.clone(),
                GroupMode::Reason => pkg.reason.label().to_string(),
                _ => self.taxonomy.label_for(pkg, mode),
            };
            if label.is_empty() {
                continue;
            }
            *counts.entry(label).or_insert(0) += 1;
        }

        let mut buckets: Vec<Bucket> = counts
            .into_iter()
            .map(|(label, count)| Bucket {
                selected: self.bucket_selected(&label),
                label,
                count,
            })
            .collect();
        buckets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));

        self.sidebar.total = self.installed.len();
        self.sidebar.buckets = buckets;
        if self.sidebar.cursor >= self.sidebar.buckets.len() {
            self.sidebar.cursor = self.sidebar.buckets.len().saturating_sub(1);
        }
    }

    fn bucket_selected(&self, label: &str) -> bool {
        match self.sidebar.mode {
            GroupMode::Category => self.filter.categories.iter().any(|c| c == label),
            GroupMode::Family => self.filter.families.iter().any(|f| f == label),
            GroupMode::Repo => self.filter.repo.as_deref() == Some(label),
            GroupMode::Reason => self
                .filter
                .reason
                .map(|r| r.label() == label)
                .unwrap_or(false),
            GroupMode::None => false,
        }
    }

    /// Move o cursor do sidebar sem alterar filtros.
    pub fn sidebar_move(&mut self, delta: isize) {
        let len = self.sidebar.buckets.len();
        if len == 0 {
            return;
        }
        let next = (self.sidebar.cursor as isize + delta).clamp(0, len as isize - 1);
        self.sidebar.cursor = next as usize;
    }

    /// Liga/desliga o grupo sob o cursor (multi-seleção).
    pub fn sidebar_toggle(&mut self) {
        let Some(bucket) = self.sidebar.current().cloned() else {
            return;
        };
        self.apply_bucket(&bucket.label, true);
        self.after_filter_change();
        self.status = format!("filtro: {}", bucket.label);
    }

    /// Aplica um grupo como filtro exclusivo (scrub com `[` / `]`).
    pub fn sidebar_scrub(&mut self, delta: isize) {
        if self.sidebar.buckets.is_empty() {
            return;
        }
        self.sidebar_move(delta);
        let Some(bucket) = self.sidebar.current().cloned() else {
            return;
        };
        self.clear_bucket_filters();
        self.apply_bucket(&bucket.label, false);
        self.after_filter_change();
        self.status = format!(
            "{}: {} ({} pacotes)",
            self.sidebar.title, bucket.label, bucket.count
        );
    }

    /// Limpa o filtro da dimensão do sidebar.
    pub fn sidebar_clear(&mut self) {
        self.clear_bucket_filters();
        self.after_filter_change();
        self.status = format!("{}: todos", self.sidebar.title);
    }

    fn apply_bucket(&mut self, label: &str, toggle: bool) {
        match self.sidebar.mode {
            GroupMode::Category => {
                if toggle {
                    self.filter.toggle_category(label);
                } else {
                    self.filter.categories = vec![label.to_string()];
                }
            }
            GroupMode::Family => {
                if toggle {
                    self.filter.toggle_family(label);
                } else {
                    self.filter.families = vec![label.to_string()];
                }
            }
            GroupMode::Repo => {
                if self.filter.repo.as_deref() == Some(label) {
                    self.filter.repo = None;
                } else {
                    self.filter.repo = Some(label.to_string());
                }
            }
            GroupMode::Reason => {
                let parsed = [
                    Reason::User,
                    Reason::Dependency,
                    Reason::WeakDependency,
                    Reason::Clean,
                ]
                .into_iter()
                .find(|r| r.label() == label);
                self.filter.reason = if self.filter.reason == parsed {
                    None
                } else {
                    parsed
                };
            }
            GroupMode::None => {}
        }
    }

    fn clear_bucket_filters(&mut self) {
        match self.sidebar.mode {
            GroupMode::Category => self.filter.categories.clear(),
            GroupMode::Family => self.filter.families.clear(),
            GroupMode::Repo => self.filter.repo = None,
            GroupMode::Reason => self.filter.reason = None,
            GroupMode::None => {}
        }
    }

    /// Recalcula sidebar + tabela após mexer em filtros.
    pub fn after_filter_change(&mut self) {
        self.refresh();
        self.rebuild_sidebar();
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar.visible = !self.sidebar.visible;
        self.status = if self.sidebar.visible {
            "sidebar visível".to_string()
        } else {
            "sidebar oculto".to_string()
        };
    }
}
