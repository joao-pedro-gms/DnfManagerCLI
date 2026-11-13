//! Subcomandos do CLI. Todos compartilham o mesmo backend da TUI.

mod categories;
mod list;
mod mutate;

pub use categories::{categories, CategoriesOptions, CategoryAction};
pub use list::{
    history, list, orphans, repos, updates, HistoryOptions, ListOptions, UpdatesOptions,
};
pub use mutate::{marks, remove, MarksOptions, RemoveOptions};

/// Códigos de saída documentados no `--help`.
pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_PROBLEMS: i32 = 2;
pub const EXIT_DENIED: i32 = 3;
pub const EXIT_NO_DAEMON: i32 = 4;

pub fn exit_code(err: &dnfm_core::BackendError) -> i32 {
    match err {
        dnfm_core::BackendError::DaemonUnavailable(_) => EXIT_NO_DAEMON,
        dnfm_core::BackendError::NotAuthorized => EXIT_DENIED,
        dnfm_core::BackendError::Blocked(_) => EXIT_PROBLEMS,
        _ => EXIT_ERROR,
    }
}

pub fn report_err(err: &dnfm_core::BackendError) -> i32 {
    eprintln!("dnfm: {err}");
    exit_code(err)
}
