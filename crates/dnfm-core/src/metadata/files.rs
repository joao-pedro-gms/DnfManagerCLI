//! Agregação da lista de arquivos em sinais de tipo + leitura de .desktop.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::FileSignals;

/// Tudo que interessa saber sobre os arquivos de um pacote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFiles {
    pub count: usize,
    /// Diretórios de topo com contagem, ordenados do maior para o menor.
    pub dirs: Vec<(String, usize)>,
    pub signals: FileSignals,
    /// Nomes dos `.desktop` que pertencem ao pacote.
    pub desktop_ids: Vec<String>,
    /// Nomes dos arquivos de metainfo (AppStream) do pacote.
    pub metainfo_ids: Vec<String>,
}

const TOP_DIRS: usize = 10;

/// Classifica caminhos em sinais e agrega diretórios.
pub fn signals_from_paths<I, S>(paths: I) -> PackageFiles
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut files = PackageFiles::default();
    let mut dirs: BTreeMap<String, usize> = BTreeMap::new();

    for path in paths {
        let path = path.as_ref();
        if path.is_empty() {
            continue;
        }
        files.count += 1;
        *dirs.entry(top_dir(path)).or_insert(0) += 1;

        if path.starts_with("/usr/bin/") || path.starts_with("/usr/sbin/") {
            files.signals.set(FileSignals::BIN);
        }
        if path.starts_with("/usr/lib64/") || path.starts_with("/usr/lib/") {
            files.signals.set(FileSignals::LIB);
        }
        if path.starts_with("/usr/include/") {
            files.signals.set(FileSignals::INCLUDE);
        }
        if path.starts_with("/usr/share/doc/") {
            files.signals.set(FileSignals::DOC);
        }
        if path.starts_with("/usr/share/man/") {
            files.signals.set(FileSignals::MAN);
        }
        if path.starts_with("/usr/share/fonts/") {
            files.signals.set(FileSignals::FONTS);
        }
        if path.starts_with("/usr/lib/systemd/") {
            files.signals.set(FileSignals::SYSTEMD);
        }
        if path.starts_with("/usr/share/applications/") && path.ends_with(".desktop") {
            files.signals.set(FileSignals::DESKTOP);
            if let Some(name) = path.rsplit('/').next() {
                files.desktop_ids.push(name.to_string());
            }
        }
        if path.starts_with("/usr/share/metainfo/") || path.starts_with("/usr/share/appdata/") {
            files.signals.set(FileSignals::METAINFO);
            if let Some(name) = path.rsplit('/').next() {
                files.metainfo_ids.push(name.to_string());
            }
        }
        if path.starts_with("/usr/share/locale/") {
            files.signals.set(FileSignals::I18N);
        }
        if path.starts_with("/usr/lib/python3") || path.starts_with("/usr/lib64/python3") {
            files.signals.set(FileSignals::PYTHON);
        }
        if path.starts_with("/usr/share/perl5/") || path.starts_with("/usr/lib64/perl5/") {
            files.signals.set(FileSignals::PERL);
        }
        if path.starts_with("/etc/") {
            files.signals.set(FileSignals::CONFIG);
        }
        if path.starts_with("/usr/lib64/pkgconfig/") || path.ends_with(".h") {
            files.signals.set(FileSignals::HEADERS);
        }
    }

    let mut dirs: Vec<(String, usize)> = dirs.into_iter().collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    dirs.truncate(TOP_DIRS);
    files.dirs = dirs;
    files
}

/// Primeiros três componentes do caminho (`/usr/share/man` para
/// `/usr/share/man/man1/foo.1.gz`).
fn top_dir(path: &str) -> String {
    let mut partes = path.split('/').filter(|p| !p.is_empty());
    match (partes.next(), partes.next(), partes.next()) {
        (Some(a), Some(b), Some(c)) => format!("/{a}/{b}/{c}"),
        (Some(a), Some(b), None) => format!("/{a}/{b}"),
        (Some(a), None, _) => format!("/{a}"),
        _ => "/".to_string(),
    }
}

/// Lê `Categories=` de um `.desktop`, ignorando grupos que não sejam o principal.
pub fn parse_desktop_file(path: &Path) -> Vec<String> {
    let Ok(texto) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_desktop_text(&texto)
}

pub fn parse_desktop_text(texto: &str) -> Vec<String> {
    let mut secao = String::new();
    for linha in texto.lines() {
        let linha = linha.trim();
        if linha.starts_with('[') && linha.ends_with(']') {
            secao = linha[1..linha.len() - 1].to_string();
            continue;
        }
        if secao != "Desktop Entry" {
            continue;
        }
        if let Some(valor) = linha.strip_prefix("Categories=") {
            return valor
                .split(';')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string)
                .collect();
        }
    }
    Vec::new()
}

/// Categorias dos `.desktop` do pacote, lendo de `dir`.
pub fn desktop_categories(dir: &Path, ids: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        let mut categorias = parse_desktop_file(&dir.join(id));
        out.append(&mut categorias);
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifica_um_pacote_de_aplicativo() {
        let files = signals_from_paths([
            "/usr/bin/vlc",
            "/usr/share/applications/vlc.desktop",
            "/usr/share/man/man1/vlc.1.gz",
            "/usr/share/locale/pt_BR/LC_MESSAGES/vlc.mo",
            "/usr/lib64/vlc/plugins/libx.so",
        ]);
        assert_eq!(files.count, 5);
        assert!(files.signals.has(FileSignals::BIN));
        assert!(files.signals.has(FileSignals::DESKTOP));
        assert!(files.signals.has(FileSignals::MAN));
        assert!(files.signals.has(FileSignals::I18N));
        assert!(files.signals.has(FileSignals::LIB));
        assert_eq!(files.desktop_ids, vec!["vlc.desktop".to_string()]);
    }

    #[test]
    fn biblioteca_sem_bin_nao_vira_comando() {
        let files = signals_from_paths(["/usr/lib64/libfoo.so.1", "/usr/lib64/pkgconfig/foo.pc"]);
        assert!(files.signals.has(FileSignals::LIB));
        assert!(!files.signals.has(FileSignals::BIN));
    }

    #[test]
    fn diretorios_de_topo_sao_agregados_e_limitados() {
        let mut paths = Vec::new();
        for i in 0..20 {
            paths.push(format!("/usr/share/man/man1/foo{i}.1.gz"));
            paths.push(format!("/usr/lib64/lib{i}.so"));
        }
        let files = signals_from_paths(&paths);
        assert!(files.dirs.len() <= TOP_DIRS);
        assert_eq!(files.dirs[0].0, "/usr/share/man");
        assert_eq!(files.dirs[0].1, 20);
    }

    #[test]
    fn le_categorias_do_desktop() {
        let texto = "[Desktop Entry]\nType=Application\nCategories=AudioVideo;Player;\nName=VLC\n";
        assert_eq!(
            parse_desktop_text(texto),
            vec!["AudioVideo".to_string(), "Player".to_string()]
        );
    }

    #[test]
    fn ignora_categorias_fora_do_grupo_principal() {
        let texto =
            "[Desktop Action new]\nCategories=Errada;\n[Desktop Entry]\nCategories=Certa;\n";
        assert_eq!(parse_desktop_text(texto), vec!["Certa".to_string()]);
    }

    #[test]
    fn arquivo_inexistente_nao_quebra() {
        assert!(parse_desktop_file(Path::new("/nao/existe.desktop")).is_empty());
    }
}
