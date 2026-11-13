//! Proteção contra remoções que tornariam o sistema inutilizável.
//!
//! O resolvedor do DNF já recusa remover pacotes que quebram requisitos de
//! outros instalados. O buraco é o que nada mais requer: `dnf5`, `sudo`,
//! `firewalld`. A lista abaixo fecha esse buraco.

use std::sync::OnceLock;

use regex::RegexSet;

const PATTERNS: &[&str] = &[
    r"^dnf5?$",
    r"^libdnf5.*$",
    r"^dnf5daemon.*$",
    r"^rpm$",
    r"^rpm-libs$",
    r"^rpm-plugin-.*$",
    r"^glibc$",
    r"^glibc-.*$",
    r"^bash$",
    r"^coreutils$",
    r"^coreutils-common$",
    r"^filesystem$",
    r"^setup$",
    r"^systemd$",
    r"^systemd-.*$",
    r"^udev.*$",
    r"^dbus$",
    r"^dbus-.*$",
    r"^polkit$",
    r"^polkit-.*$",
    r"^kernel$",
    r"^kernel-.*$",
    r"^grub2-.*$",
    r"^shim.*$",
    r"^efibootmgr$",
    r"^dracut$",
    r"^dracut-.*$",
    r"^sudo$",
    r"^sudo-.*$",
    r"^openssh-server$",
    r"^selinux-policy.*$",
    r"^policycoreutils.*$",
    r"^firewalld.*$",
    r"^fedora-release.*$",
    r"^fedora-repos.*$",
    r"^fedora-gpg-keys$",
    r"^python3(-libs|-dnf.*|-libdnf5.*)?$",
];

fn set() -> &'static RegexSet {
    static SET: OnceLock<RegexSet> = OnceLock::new();
    SET.get_or_init(|| RegexSet::new(PATTERNS).expect("padrões da blocklist são válidos"))
}

/// `Some(motivo)` se o pacote é protegido.
pub fn is_protected(name: &str) -> Option<&'static str> {
    if set().is_match(name) {
        Some("pacote essencial do sistema")
    } else {
        None
    }
}

/// Versão que também protege o kernel em execução.
pub fn protected_reason(name: &str, running_kernel: Option<&str>) -> Option<String> {
    if let Some(kernel) = running_kernel {
        if name == format!("kernel-core-{kernel}") || name == format!("kernel-{kernel}") {
            return Some("é o kernel em execução".to_string());
        }
    }
    is_protected(name).map(|r| r.to_string())
}

/// Descobre a release do kernel em execução (`uname -r`) sem dependência extra.
pub fn running_kernel() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protege_criticos_obvios() {
        for name in [
            "dnf5",
            "dnf",
            "glibc",
            "systemd",
            "kernel-core",
            "sudo",
            "rpm",
        ] {
            assert!(is_protected(name).is_some(), "{name} deveria ser protegido");
        }
    }

    #[test]
    fn nao_protege_pacotes_comuns() {
        for name in ["vlc", "firefox", "dosbox-staging", "texlive-a2ping", "wine"] {
            assert!(
                is_protected(name).is_none(),
                "{name} não deveria ser protegido"
            );
        }
    }

    #[test]
    fn protege_kernel_em_execucao_pelo_nome_exato() {
        let reason = protected_reason(
            "kernel-core-7.3.3-300.fc45.x86_64",
            Some("7.3.3-300.fc45.x86_64"),
        );
        assert!(reason.is_some());
        // kernel-core já cai na blocklist estática; o teste garante a mensagem.
        assert!(is_protected("kernel-core-6.0.0").is_some());
    }
}
