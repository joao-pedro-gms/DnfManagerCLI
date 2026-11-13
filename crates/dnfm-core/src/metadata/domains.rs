//! Família de projeto a partir do domínio da URL do pacote.
//!
//! `%{URL}` está preenchida em praticamente todo pacote do Fedora e o domínio
//! identifica o projeto melhor que o prefixo do nome (4.786 pacotes apontam
//! para `tug.org`, 254 para `invent.kde.org`). Domínios genéricos
//! (`github.com`, `fedoraproject.org`) devolvem `None` para não criar famílias
//! falsas — nesses casos a heurística de prefixo continua valendo.

const DOMAINS: &[(&str, &str)] = &[
    ("tug.org", "TeX Live"),
    ("ctan.org", "TeX Live"),
    ("latex-project.org", "TeX Live"),
    ("invent.kde.org", "KDE Plasma"),
    ("kde.org", "KDE Plasma"),
    ("gnome.org", "GNOME"),
    ("gtk.org", "GNOME"),
    ("metacpan.org", "Perl"),
    ("cpan.org", "Perl"),
    ("python.org", "Python"),
    ("pypi.org", "Python"),
    ("gnu.org", "GNU"),
    ("x.org", "Xorg"),
    ("freedesktop.org", "Sistema base"),
    ("kernel.org", "Kernel e boot"),
    ("gstreamer.freedesktop.org", "Multimídia"),
    ("ffmpeg.org", "Multimídia"),
    ("videolan.org", "Multimídia"),
    ("xiph.org", "Multimídia"),
    ("winehq.org", "Wine"),
    ("libreoffice.org", "LibreOffice"),
    ("documentfoundation.org", "LibreOffice"),
    ("openjdk.java.net", "Java"),
    ("java.net", "Java"),
    ("rust-lang.org", "Rust"),
    ("golang.org", "Go"),
    ("go.dev", "Go"),
    ("nodejs.org", "Node.js"),
    ("php.net", "PHP"),
    ("ruby-lang.org", "Ruby"),
    ("docker.com", "Contêineres"),
    ("podman.io", "Contêineres"),
    ("containers.io", "Contêineres"),
    ("postgresql.org", "Banco de dados"),
    ("mariadb.org", "Banco de dados"),
    ("mysql.com", "Banco de dados"),
    ("sqlite.org", "Banco de dados"),
    ("redis.io", "Banco de dados"),
    ("apache.org", "Servidores"),
    ("nginx.org", "Servidores"),
    ("openssl.org", "Segurança"),
    ("gnupg.org", "Segurança"),
    ("selinuxproject.org", "Segurança"),
    ("rpm.org", "Sistema base"),
    ("systemd.io", "Sistema base"),
    ("qt.io", "Qt"),
    ("boost.org", "Desenvolvimento"),
    ("cmake.org", "Desenvolvimento"),
    ("mesonbuild.com", "Desenvolvimento"),
    ("opensuse.org", "Sistema base"),
];

/// Domínios que não identificam projeto e não devem gerar família.
const GENERICOS: &[&str] = &[
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "fedoraproject.org",
    "src.fedoraproject.org",
    "pagure.io",
    "copr.fedorainfracloud.org",
    "sourceforge.net",
    "gitlab.gnome.org",
];

pub fn family_from_url(url: &str) -> Option<&'static str> {
    let host = host_of(url)?;
    if GENERICOS.iter().any(|g| host == *g) {
        return None;
    }
    DOMAINS
        .iter()
        .find(|(dominio, _)| host == *dominio || host.ends_with(&format!(".{dominio}")))
        .map(|(_, familia)| *familia)
}

/// Extrai o host de uma URL, tolerando ausência de esquema e sufixos.
pub fn host_of(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let sem_esquema = url.split("://").last().unwrap_or(url);
    let host = sem_esquema.split('/').next()?.split('?').next()?;
    let host = host.rsplit('@').next()?.split(':').next()?;
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_projetos_conhecidos() {
        assert_eq!(
            family_from_url("https://www.tug.org/texlive/"),
            Some("TeX Live")
        );
        assert_eq!(
            family_from_url("https://invent.kde.org/plasma/plasma-desktop"),
            Some("KDE Plasma")
        );
        assert_eq!(
            family_from_url("https://metacpan.org/dist/Getopt-Long"),
            Some("Perl")
        );
        assert_eq!(family_from_url("https://qemu.org"), None);
    }

    #[test]
    fn dominio_generico_nao_vira_familia() {
        assert_eq!(family_from_url("https://github.com/rust-lang/rust"), None);
        assert_eq!(family_from_url("https://fedoraproject.org"), None);
        assert_eq!(
            family_from_url("https://src.fedoraproject.org/rpms/vlc"),
            None
        );
    }

    #[test]
    fn subdominio_casa_com_o_dominio_raiz() {
        assert_eq!(
            family_from_url("https://download.tug.org/x"),
            Some("TeX Live")
        );
        assert_eq!(
            family_from_url("https://lists.gnu.org/mailman"),
            Some("GNU")
        );
    }

    #[test]
    fn tolera_urls_estranhas() {
        assert_eq!(host_of("tug.org/texlive"), Some("tug.org".to_string()));
        assert_eq!(
            host_of("https://user@host.example:8080/x"),
            Some("host.example".to_string())
        );
        assert_eq!(host_of(""), None);
        assert_eq!(host_of("sem-dominio"), None);
    }
}
