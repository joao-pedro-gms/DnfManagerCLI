//! Testes do catálogo de tags e do mapeamento de categorias.

use super::*;
use crate::model::Reason;

fn pkg(name: &str) -> Package {
    Package {
        name: name.into(),
        evr: "1-1.fc45".into(),
        arch: "x86_64".into(),
        repo_id: "@System".into(),
        from_repo_id: None,
        size: 1024,
        install_time: 0,
        summary: String::new(),
        reason: Reason::User,
        nevra: format!("{name}-1-1.fc45.x86_64"),
        is_installed: true,
    }
}

#[test]
fn familias_sozinhas_tornam_o_catalogo_nao_vazio() {
    let mut catalog = TagCatalog::default();
    catalog.families.insert("foo".into(), "Projeto Foo".into());
    assert!(!catalog.is_empty());
}

#[test]
fn appstream_tem_prioridade_sobre_tudo() {
    let mut catalog = TagCatalog::default();
    catalog.appstream.insert(
        "vlc".into(),
        AppStreamComponent {
            id: "vlc.desktop".into(),
            categories: vec!["Player".into(), "AudioVideo".into()],
            summary: None,
        },
    );
    let tag = catalog.category_for(&pkg("vlc")).unwrap();
    assert_eq!(tag.label, "Multimídia");
    assert_eq!(tag.source, TagSource::AppStream);
    assert!(tag.confidence() >= 90);
}

#[test]
fn desktop_e_fallback_do_appstream() {
    let mut catalog = TagCatalog::default();
    catalog
        .desktop
        .insert("foo".into(), vec!["Graphics".into(), "Viewer".into()]);
    let tag = catalog.category_for(&pkg("foo")).unwrap();
    assert_eq!(tag.label, "Gráficos");
    assert_eq!(tag.source, TagSource::DesktopFile);
}

#[test]
fn sinais_de_arquivo_viram_tags() {
    let mut files = PackageFiles::default();
    files.signals.set(FileSignals::BIN);
    files.signals.set(FileSignals::MAN);
    let mut catalog = TagCatalog::default();
    catalog.files.insert("ripgrep".into(), files);
    let tags = catalog.tags_for(&pkg("ripgrep"));
    let labels: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
    assert!(labels.contains(&"comando"));
    assert!(labels.contains(&"man"));
    assert!(tags.iter().all(|t| t.source == TagSource::FileSignal));
}

#[test]
fn sem_metadado_nao_inventa_categoria() {
    let catalog = TagCatalog::default();
    assert!(catalog.category_for(&pkg("qualquer")).is_none());
    assert!(catalog.tags_for(&pkg("qualquer")).is_empty());
}

#[test]
fn categoria_principal_segue_prioridade_freedesktop() {
    let comp = AppStreamComponent {
        id: "x".into(),
        categories: vec!["Utility".into(), "Graphics".into(), "Viewer".into()],
        summary: None,
    };
    assert_eq!(comp.primary_category(), Some("Graphics"));
}

#[test]
fn rotulos_sao_estaveis() {
    assert_eq!(category_label("AudioVideo"), "Multimídia");
    assert_eq!(category_label("Development"), "Desenvolvimento");
    assert_eq!(category_label("Inedita"), "Inedita");
}

#[test]
fn contagem_de_tags_agrega_por_rotulo() {
    let packages = vec![pkg("a"), pkg("b")];
    let mut catalog = TagCatalog::default();
    for name in ["a", "b"] {
        let mut files = PackageFiles::default();
        files.signals.set(FileSignals::BIN);
        catalog.files.insert(name.to_string(), files);
    }
    let counts = tag_counts(&packages, &catalog);
    assert_eq!(counts.get("comando"), Some(&2));
}
