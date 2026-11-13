//! Tipos de estado da TUI: abas, modais, comandos e eventos.

use dnfm_core::{
    ConfirmToken, HistoryEntry, Outcome, Package, PackageDetails, Preview, ProgressEvent, Repo,
    Upgrade,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Installed,
    Updates,
    Orphans,
    History,
    Repos,
    Stats,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Installed,
        Tab::Updates,
        Tab::Orphans,
        Tab::History,
        Tab::Repos,
        Tab::Stats,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Installed => "Instalados",
            Tab::Updates => "Atualizações",
            Tab::Orphans => "Órfãos",
            Tab::History => "Histórico",
            Tab::Repos => "Repos",
            Tab::Stats => "Estatísticas",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn step(self, delta: isize) -> Self {
        let len = Self::ALL.len() as isize;
        let idx = (self.index() as isize + delta).rem_euclid(len);
        Self::ALL[idx as usize]
    }

    /// Só estas abas listam pacotes que podem ser marcados.
    pub fn marks_packages(self) -> bool {
        matches!(self, Tab::Installed | Tab::Orphans)
    }
}

#[derive(Debug, Clone)]
pub enum Modal {
    Help,
    Preview(Preview),
    Progress,
    Error(String),
    Inspector,
}

#[derive(Debug, Default)]
pub struct Progress {
    pub total: u64,
    pub done: u64,
    pub current: String,
    pub log: Vec<String>,
    pub success: Option<bool>,
}

/// Efeito colateral a executar pelo laço de eventos.
#[derive(Debug)]
pub enum Cmd {
    LoadAll,
    /// Carrega metadados automáticos (arquivos + AppStream), com cache.
    LoadMetadata(u64),
    LoadHistory,
    LoadOrphans,
    LoadDetails(String),
    Preview(Vec<String>),
    PreviewUpgrade(Vec<String>),
    Execute(ConfirmToken),
    CancelPreview,
    Quit,
}

/// Resultado assíncrono devolvido ao estado.
#[derive(Debug)]
pub enum AppEvent {
    Installed(Vec<Package>),
    Metadata(Box<dnfm_core::metadata::TagCatalog>),
    Upgrades(Vec<Upgrade>),
    Orphans(Vec<Package>),
    History(Vec<HistoryEntry>),
    Repos(Vec<Repo>),
    Details(PackageDetails),
    Preview(Box<Preview>),
    Executed(Outcome),
    Progress(ProgressEvent),
    Failed(String),
}
