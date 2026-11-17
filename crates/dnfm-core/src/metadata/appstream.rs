//! Leitura do catálogo AppStream do sistema.
//!
//! O catálogo compilado (`/usr/share/swcatalog/xml/*.xml.gz`) traz, por
//! componente, o `<pkgname>` do RPM que o fornece e as `<categories>`. É a única
//! fonte de categoria funcional oficial disponível offline.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use super::AppStreamComponent;

/// Diretório padrão dos catálogos.
pub fn default_catalog_dirs() -> Vec<PathBuf> {
    vec![PathBuf::from("/usr/share/swcatalog/xml")]
}

/// Lê todos os catálogos de um diretório, ignorando os ilegíveis.
pub fn parse_catalog_dir(dir: &Path) -> HashMap<String, AppStreamComponent> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let caminho = entry.path();
        let nome = caminho.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !(nome.ends_with(".xml") || nome.ends_with(".xml.gz")) {
            continue;
        }
        match parse_catalog_file(&caminho) {
            Ok(componentes) => out.extend(componentes),
            Err(err) => {
                tracing::warn!(arquivo = %caminho.display(), %err, "catálogo AppStream ilegível")
            }
        }
    }
    out
}

pub fn parse_catalog_file(path: &Path) -> Result<HashMap<String, AppStreamComponent>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader: Box<dyn Read> = if path.extension().and_then(|e| e.to_str()) == Some("gz") {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    parse_catalog(BufReader::new(reader))
}

fn aplica_texto(
    valor: String,
    stack: &[String],
    comp: &mut Option<AppStreamComponent>,
    pkgname: &mut Option<String>,
    in_categories: bool,
) {
    let atual = stack.last().map(String::as_str).unwrap_or("");
    if let Some(component) = comp.as_mut() {
        match atual {
            "id" => component.id = valor.trim().to_string(),
            "summary" => component
                .summary
                .get_or_insert_with(String::new)
                .push_str(&valor),
            "pkgname" => *pkgname = Some(valor.trim().to_string()),
            // Componentes sem <pkgname> costumam declarar o RPM em <provides><binary>.
            "binary" if pkgname.is_none() => *pkgname = Some(valor.trim().to_string()),
            "category" if in_categories => component.categories.push(valor.trim().to_string()),
            _ => {}
        }
    }
}

fn resolve_general_ref(referencia: quick_xml::events::BytesRef<'_>) -> Option<String> {
    if let Ok(Some(caractere)) = referencia.resolve_char_ref() {
        return Some(caractere.to_string());
    }
    let entidade = referencia.decode().ok()?;
    let valor = match entidade.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "quot" => "\"",
        _ => return None,
    };
    Some(valor.to_string())
}

/// Parser por streaming: o catálogo do Fedora tem dezenas de MB.
pub fn parse_catalog<R: std::io::BufRead>(
    reader: R,
) -> Result<HashMap<String, AppStreamComponent>, String> {
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut out = HashMap::new();

    let mut stack: Vec<String> = Vec::new();
    let mut comp: Option<AppStreamComponent> = None;
    let mut pkgname: Option<String> = None;
    let mut in_categories = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let nome = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if nome == "component" {
                    comp = Some(AppStreamComponent {
                        id: String::new(),
                        categories: Vec::new(),
                        summary: None,
                    });
                    pkgname = None;
                } else if nome == "categories" {
                    in_categories = true;
                }
                stack.push(nome);
            }
            Ok(Event::Text(texto)) => {
                let valor = texto
                    .decode()
                    .ok()
                    .and_then(|valor| {
                        quick_xml::escape::unescape(&valor)
                            .ok()
                            .map(|valor| valor.into_owned())
                    })
                    .unwrap_or_default();
                aplica_texto(valor, &stack, &mut comp, &mut pkgname, in_categories);
            }
            Ok(Event::GeneralRef(referencia)) => {
                if let Some(valor) = resolve_general_ref(referencia) {
                    aplica_texto(valor, &stack, &mut comp, &mut pkgname, in_categories);
                }
            }
            Ok(Event::End(e)) => {
                let nome = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if nome == "categories" {
                    in_categories = false;
                }
                if nome == "component" {
                    if let (Some(component), Some(pkg)) = (comp.take(), pkgname.take()) {
                        if !pkg.is_empty() {
                            // Vários componentes podem apontar para o mesmo RPM:
                            // acumula categorias em vez de sobrescrever.
                            let entrada = out.entry(pkg).or_insert_with(|| AppStreamComponent {
                                id: component.id.clone(),
                                categories: Vec::new(),
                                summary: component.summary.clone(),
                            });
                            for categoria in component.categories {
                                if !entrada.categories.contains(&categoria) {
                                    entrada.categories.push(categoria);
                                }
                            }
                        }
                    }
                    pkgname = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
        buf.clear();
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const CATALOGO: &str = r#"<?xml version="1.0"?>
<components version="0.16">
  <component type="desktop-application">
    <id>org.videolan.VLC.desktop</id>
    <pkgname>vlc</pkgname>
    <summary>Player de mídia</summary>
    <categories><category>AudioVideo</category><category>Player</category></categories>
  </component>
  <component type="desktop-application">
    <id>org.gimp.GIMP.desktop</id>
    <pkgname>gimp</pkgname>
    <categories><category>Graphics</category></categories>
  </component>
  <component type="addon">
    <id>org.gimp.GIMP.Plugin.Foo</id>
    <pkgname>gimp</pkgname>
    <categories><category>Addons</category></categories>
  </component>
  <component type="font">
    <id>org.example.Font</id>
    <categories><category>Fonts</category></categories>
    <provides><binary>example-fonts</binary></provides>
  </component>
</components>"#;

    #[test]
    fn le_pkgname_e_categorias() {
        let mapa = parse_catalog(Cursor::new(CATALOGO)).expect("parse");
        assert_eq!(mapa.len(), 3);
        let vlc = &mapa["vlc"];
        assert_eq!(vlc.categories, vec!["AudioVideo", "Player"]);
        assert_eq!(vlc.summary.as_deref(), Some("Player de mídia"));
    }

    #[test]
    fn acumula_categorias_do_mesmo_pacote() {
        let mapa = parse_catalog(Cursor::new(CATALOGO)).expect("parse");
        let gimp = &mapa["gimp"];
        assert!(gimp.categories.contains(&"Graphics".to_string()));
        assert!(gimp.categories.contains(&"Addons".to_string()));
    }

    #[test]
    fn usa_provides_binary_quando_falta_pkgname() {
        let mapa = parse_catalog(Cursor::new(CATALOGO)).expect("parse");
        assert!(mapa.contains_key("example-fonts"));
    }

    #[test]
    fn decodifica_entidades_no_texto() {
        let xml = r#"<components><component><pkgname>pacote</pkgname><summary>A &amp; B &#x2D; C</summary></component></components>"#;
        let mapa = parse_catalog(Cursor::new(xml)).expect("parse");
        assert_eq!(mapa["pacote"].summary.as_deref(), Some("A & B - C"));
    }

    #[test]
    fn catalogo_vazio_nao_falha() {
        let mapa = parse_catalog(Cursor::new("<components></components>")).expect("parse");
        assert!(mapa.is_empty());
    }

    #[test]
    fn diretorio_inexistente_devolve_vazio() {
        assert!(parse_catalog_dir(Path::new("/nao/existe")).is_empty());
    }
}
