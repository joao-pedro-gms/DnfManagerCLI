//! Tipos auxiliares de agrupamento e edição de filtros.

use dnfm_core::categories::GroupMode;

/// Uma linha da tabela: cabeçalho de grupo ou pacote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowRef {
    Header {
        label: String,
        count: usize,
        collapsed: bool,
    },
    Package {
        index: usize,
    },
}

impl RowRef {
    pub fn is_header(&self) -> bool {
        matches!(self, RowRef::Header { .. })
    }
}

/// Painel de filtros: mantém apenas a posição do cursor; os valores vivem no
/// `Filter` do `App`.
#[derive(Debug, Clone, Default)]
pub struct FilterPanel {
    pub cursor: usize,
}

/// Facetas editáveis do painel de filtros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterFacet {
    Text,
    Category,
    Family,
    Repo,
    Reason,
    Arch,
    Size,
    Date,
    Marked,
    Protected,
    Updates,
    Clear,
}

impl FilterFacet {
    pub const ALL: [FilterFacet; 12] = [
        FilterFacet::Text,
        FilterFacet::Category,
        FilterFacet::Family,
        FilterFacet::Repo,
        FilterFacet::Reason,
        FilterFacet::Arch,
        FilterFacet::Size,
        FilterFacet::Date,
        FilterFacet::Marked,
        FilterFacet::Protected,
        FilterFacet::Updates,
        FilterFacet::Clear,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "texto",
            Self::Category => "categoria",
            Self::Family => "família",
            Self::Repo => "repo",
            Self::Reason => "motivo",
            Self::Arch => "arquitetura",
            Self::Size => "tamanho",
            Self::Date => "instalado em",
            Self::Marked => "só marcados",
            Self::Protected => "só protegidos",
            Self::Updates => "com atualização",
            Self::Clear => "limpar tudo",
        }
    }

    /// Facetas que abrem um seletor em vez de ciclar valores.
    pub fn opens_picker(self) -> Option<PickerTarget> {
        match self {
            Self::Category => Some(PickerTarget::Category),
            Self::Family => Some(PickerTarget::Family),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerTarget {
    Category,
    Family,
}

impl PickerTarget {
    pub fn title(self) -> &'static str {
        match self {
            Self::Category => "filtrar por categoria",
            Self::Family => "filtrar por família",
        }
    }

    pub fn group_mode(self) -> GroupMode {
        match self {
            Self::Category => GroupMode::Category,
            Self::Family => GroupMode::Family,
        }
    }
}

/// Seletor multi-escolha com busca, usado para categoria e família.
#[derive(Debug, Clone, Default)]
pub struct Picker {
    pub target_items: Vec<(String, usize)>,
    pub selected: Vec<String>,
    pub cursor: usize,
    pub query: String,
    pub target: Option<PickerTarget>,
}

impl Picker {
    pub fn new(target: PickerTarget, items: Vec<(String, usize)>, selected: Vec<String>) -> Self {
        let mut picker = Self {
            target_items: items,
            selected,
            cursor: 0,
            query: String::new(),
            target: Some(target),
        };
        picker.refocus();
        picker
    }

    /// Itens que casam com a busca atual.
    pub fn filtered(&self) -> Vec<(String, usize)> {
        let q = self.query.to_ascii_lowercase();
        self.target_items
            .iter()
            .filter(|(label, _)| q.is_empty() || label.to_ascii_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    pub fn current(&self) -> Option<(String, usize)> {
        self.filtered().get(self.cursor).cloned()
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.filtered().len();
        if len == 0 {
            self.cursor = 0;
            return;
        }
        let next = (self.cursor as isize + delta).clamp(0, len as isize - 1);
        self.cursor = next as usize;
    }

    pub fn toggle_current(&mut self) {
        if let Some((label, _)) = self.current() {
            if let Some(pos) = self.selected.iter().position(|s| *s == label) {
                self.selected.remove(pos);
            } else {
                self.selected.push(label);
            }
        }
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.refocus();
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.refocus();
    }

    fn refocus(&mut self) {
        if self.cursor >= self.filtered().len() {
            self.cursor = self.filtered().len().saturating_sub(1);
        }
    }
}
