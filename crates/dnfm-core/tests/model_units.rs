//! Testes de unidade do modelo de domínio.
//!
//! Ficam fora do `src` para manter os arquivos abaixo de 500 linhas.

use dnfm_core::model::{
    format_date, format_size, Action, Package, PlannedPackage, Preview, Reason, ResolveResult,
};

#[test]
fn formata_tamanhos() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1024), "1.0 KiB");
    assert_eq!(format_size(16_417_176), "15.7 MiB");
}

#[test]
fn formata_datas() {
    assert_eq!(format_date(0), "—");
    assert_eq!(format_date(1_793_727_256), "2026-11-03");
}

#[test]
fn preview_bloqueado_nao_confirma() {
    let preview = Preview {
        items: vec![],
        problems: vec!["quebrou".into()],
        result: ResolveResult::Failed,
    };
    assert!(preview.confirm().is_err());
}

#[test]
fn preview_vazio_nao_confirma() {
    let preview = Preview {
        items: vec![],
        problems: vec![],
        result: ResolveResult::Ok,
    };
    assert!(preview.confirm().is_err());
}

#[test]
fn preview_valido_confirma() {
    let pkg = Package {
        name: "dosbox-staging".into(),
        evr: "0.82.2-6.fc45".into(),
        arch: "x86_64".into(),
        repo_id: "@System".into(),
        from_repo_id: None,
        size: 16_417_176,
        install_time: 0,
        summary: String::new(),
        reason: Reason::User,
        nevra: "dosbox-staging-0.82.2-6.fc45.x86_64".into(),
        is_installed: true,
    };
    let preview = Preview {
        items: vec![PlannedPackage {
            action: Action::Remove,
            reason: Reason::User,
            package: pkg,
        }],
        problems: vec![],
        result: ResolveResult::Ok,
    };
    assert_eq!(preview.freed_bytes(), 16_417_176);
    let token = preview.confirm().expect("deveria confirmar");
    assert_eq!(token.removals(), 1);
}
