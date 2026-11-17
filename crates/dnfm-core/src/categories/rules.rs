use crate::model::Package;

/// Apelidos de prefixo: juntam pacotes que pertencem ao mesmo projeto.
const ALIASES: &[(&str, &str)] = &[
    ("texlive", "TeX Live"),
    ("tex", "TeX Live"),
    ("perl", "Perl"),
    ("python", "Python"),
    ("python3", "Python"),
    ("python2", "Python"),
    ("pypy", "Python"),
    ("kf5", "KDE Plasma"),
    ("kf6", "KDE Plasma"),
    ("plasma", "KDE Plasma"),
    ("kwin", "KDE Plasma"),
    ("kde", "KDE Plasma"),
    ("qt5", "Qt"),
    ("qt6", "Qt"),
    ("qt", "Qt"),
    ("gnome", "GNOME"),
    ("gtk", "GNOME"),
    ("gsettings", "GNOME"),
    ("wine", "Wine"),
    ("java", "Java"),
    ("openjdk", "Java"),
    ("jdk", "Java"),
    ("jre", "Java"),
    ("maven", "Java"),
    ("golang", "Go"),
    ("go", "Go"),
    ("rust", "Rust"),
    ("cargo", "Rust"),
    ("nodejs", "Node.js"),
    ("npm", "Node.js"),
    ("node", "Node.js"),
    ("php", "PHP"),
    ("ruby", "Ruby"),
    ("gem", "Ruby"),
    ("libreoffice", "LibreOffice"),
    ("autocorr", "LibreOffice"),
    ("docker", "Contêineres"),
    ("podman", "Contêineres"),
    ("buildah", "Contêineres"),
    ("skopeo", "Contêineres"),
    ("containerd", "Contêineres"),
    ("runc", "Contêineres"),
    ("crun", "Contêineres"),
    ("conmon", "Contêineres"),
    ("xorg", "Xorg"),
    ("xkeyboard", "Xorg"),
    ("mesa", "Gráficos"),
    ("vulkan", "Gráficos"),
    ("ffmpeg", "Multimídia"),
    ("gstreamer", "Multimídia"),
    ("vlc", "Multimídia"),
    ("mpv", "Multimídia"),
    ("pipewire", "Multimídia"),
    ("pulseaudio", "Multimídia"),
    ("alsa", "Multimídia"),
    ("systemd", "Sistema base"),
    ("udev", "Sistema base"),
    ("dbus", "Sistema base"),
    ("polkit", "Sistema base"),
    ("kernel", "Kernel e boot"),
    ("dracut", "Kernel e boot"),
    ("grub2", "Kernel e boot"),
    ("shim", "Kernel e boot"),
    ("selinux", "Segurança"),
    ("policycoreutils", "Segurança"),
    ("firewalld", "Segurança"),
    ("gnupg", "Segurança"),
    ("crypto", "Segurança"),
    ("openssl", "Segurança"),
];

/// Regras por palavra-chave, aplicadas ao nome (mais específicas primeiro).
const KEYWORDS: &[(&str, &str)] = &[
    ("firmware", "Firmware"),
    ("font", "Fontes"),
    ("langpack", "Idiomas"),
    ("-devel", "Desenvolvimento"),
    ("-headers", "Desenvolvimento"),
    ("-doc", "Documentação"),
    ("-debug", "Depuração"),
    ("-libs", "Bibliotecas"),
    ("-static", "Bibliotecas"),
    ("theme", "Temas"),
    ("icon", "Temas"),
    ("wallpaper", "Temas"),
    ("backgrounds", "Temas"),
    ("-plugin", "Plugins"),
];

pub(super) const FALLBACK_CATEGORY: &str = "Outros";
pub(super) fn prefix(name: &str) -> &str {
    name.split('-').next().unwrap_or(name)
}

pub(super) fn alias_for(prefix: &str) -> Option<&'static str> {
    if let Some(label) = lookup_alias(prefix) {
        return Some(label);
    }
    // `gtk3`, `gtk4`, `qt6`: o sufixo numérico não muda a família.
    let trimmed = prefix.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != prefix && !trimmed.is_empty() {
        return lookup_alias(trimmed);
    }
    None
}

fn lookup_alias(prefix: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, label)| *label)
}

/// Categoria curada de um pacote (grossa).
pub fn categorize(pkg: &Package) -> String {
    let name = pkg.name.to_ascii_lowercase();
    if let Some(label) = alias_for(prefix(&name)) {
        return label.to_string();
    }
    for (needle, label) in KEYWORDS {
        if name.contains(needle) {
            return label.to_string();
        }
    }
    if name.starts_with("lib") {
        return "Bibliotecas".to_string();
    }
    FALLBACK_CATEGORY.to_string()
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Família de um pacote (fina), a partir do prefixo do nome.
pub fn family(pkg: &Package) -> String {
    let name = pkg.name.to_ascii_lowercase();
    let prefix = prefix(&name);
    alias_for(prefix)
        .map(str::to_string)
        .unwrap_or_else(|| capitalized(prefix))
}
