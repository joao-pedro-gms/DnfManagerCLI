//! dnfm-core — domínio, backend D-Bus e utilidades do gerenciador de pacotes.
//!
//! A interface (TUI ou CLI) consome apenas [`DnfBackend`]; nada aqui depende de
//! terminal, o que mantém os testes rápidos.

pub mod attrs;
pub mod backend;
pub mod decode;
pub mod marks;
pub mod model;
pub mod query;
pub mod safety;

#[cfg(feature = "dbus")]
pub mod dbus;

pub mod fake;

pub use backend::{BackendError, DnfBackend};
pub use marks::{Mark, MarkSet};
pub use model::{
    format_date, format_size, Action, Advisory, ConfirmToken, HistoryEntry, Outcome, Package,
    PackageDetails, PlannedPackage, Preview, ProgressEvent, Reason, Repo, ResolveResult, Severity,
    Upgrade,
};
pub use query::{select, Filter, SortKey};

/// Dica de como subir o daemon, mostrada quando a conexão falha.
pub const DAEMON_HINT: &str = "systemctl start dnf5daemon-server";
