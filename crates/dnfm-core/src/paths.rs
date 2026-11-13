//! Caminhos de estado do usuário (XDG).

use std::path::PathBuf;

/// `$XDG_STATE_HOME/dnfm`, com fallback para `~/.local/state/dnfm`.
pub fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(dir).join("dnfm");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/state/dnfm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respeita_xdg_state_home() {
        std::env::set_var("XDG_STATE_HOME", "/tmp/xdg-teste");
        assert_eq!(state_dir(), PathBuf::from("/tmp/xdg-teste/dnfm"));
        std::env::remove_var("XDG_STATE_HOME");
    }
}
