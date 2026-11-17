//! Caminhos de estado do usuário (XDG).

use std::ffi::OsString;
use std::path::PathBuf;

/// `$XDG_STATE_HOME/dnfm`, com fallback para `~/.local/state/dnfm`.
///
/// `DNFM_STATE_HOME` tem prioridade e aponta o diretório final, sem o sufixo
/// `dnfm`: é o que mantém o modo `--fake` longe do estado real.
pub fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DNFM_STATE_HOME") {
        return PathBuf::from(dir);
    }
    state_dir_from(std::env::var_os("XDG_STATE_HOME"))
}

fn state_dir_from(xdg: Option<OsString>) -> PathBuf {
    if let Some(dir) = xdg {
        return PathBuf::from(dir).join("dnfm");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/state/dnfm")
}

/// `$XDG_CACHE_HOME/dnfm`, com fallback para `~/.cache/dnfm`.
///
/// `DNFM_CACHE_HOME` tem prioridade e aponta o diretório final, sem o sufixo
/// `dnfm`.
pub fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DNFM_CACHE_HOME") {
        return PathBuf::from(dir);
    }
    cache_dir_from(std::env::var_os("XDG_CACHE_HOME"))
}

fn cache_dir_from(xdg: Option<OsString>) -> PathBuf {
    if let Some(dir) = xdg {
        return PathBuf::from(dir).join("dnfm");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/dnfm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respeita_xdg_state_home_sem_mutar_ambiente_global() {
        assert_eq!(
            state_dir_from(Some(OsString::from("/tmp/xdg-teste"))),
            PathBuf::from("/tmp/xdg-teste/dnfm")
        );
    }

    #[test]
    fn respeita_xdg_cache_home_sem_mutar_ambiente_global() {
        assert_eq!(
            cache_dir_from(Some(OsString::from("/tmp/xdg-cache-teste"))),
            PathBuf::from("/tmp/xdg-cache-teste/dnfm")
        );
    }
}
