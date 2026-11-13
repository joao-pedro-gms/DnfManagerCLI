//! Testes de transição de estado da TUI (sem terminal, sem D-Bus).

use dnfm_core::{Action, Package, ProgressEvent, Reason};
use dnfm_tui::app::{App, Cmd, Modal, Tab};
fn pkg(name: &str, reason: Reason) -> Package {
    Package {
        name: name.into(),
        evr: "1-1.fc45".into(),
        arch: "x86_64".into(),
        repo_id: "@System".into(),
        from_repo_id: None,
        size: 1024,
        install_time: 0,
        summary: String::new(),
        reason,
        nevra: format!("{name}-1-1.fc45.x86_64"),
        is_installed: true,
    }
}

fn app_with(packages: Vec<Package>) -> App {
    let mut app = App {
        installed: packages,
        ..Default::default()
    };
    app.refresh();
    app
}

#[test]
fn navega_sem_sair_dos_limites() {
    let mut app = app_with(vec![pkg("a", Reason::User), pkg("b", Reason::User)]);
    app.move_selection(-5);
    assert_eq!(app.selected_index(), 0);
    app.move_selection(99);
    assert_eq!(app.selected_index(), 1);
}

#[test]
fn protege_pacote_critico_ao_marcar() {
    let mut app = app_with(vec![pkg("glibc", Reason::Dependency)]);
    app.toggle_mark_current();
    assert_eq!(app.marks.len(), 0);
    assert!(app.status.contains("protegido"));
}

#[test]
fn marca_e_conta_tamanho_do_pedido() {
    let mut app = app_with(vec![pkg("vlc", Reason::User)]);
    app.toggle_mark_current();
    assert_eq!(app.marks.len(), 1);
    assert!(matches!(app.request_preview(), Some(Cmd::Preview(v)) if v == vec!["vlc"]));
}

#[test]
fn marcar_todos_pula_protegidos() {
    let mut app = app_with(vec![
        pkg("vlc", Reason::User),
        pkg("systemd", Reason::Dependency),
        pkg("firefox", Reason::User),
    ]);
    app.mark_all_visible();
    assert_eq!(app.marks.len(), 2);
}

#[test]
fn preview_bloqueado_nao_gera_execucao() {
    let mut app = app_with(vec![pkg("glibc", Reason::Dependency)]);
    app.modal = Some(Modal::Preview(dnfm_core::Preview {
        items: Vec::new(),
        problems: vec!["quebrou".into()],
        result: dnfm_core::ResolveResult::Failed,
    }));
    assert!(app.confirm_preview().is_none());
    assert!(matches!(app.modal, Some(Modal::Error(_))));
}

#[test]
fn progresso_monta_log_da_transacao() {
    let mut app = app_with(vec![]);
    app.apply_progress(ProgressEvent::Begin { total: 2 });
    app.apply_progress(ProgressEvent::ActionStart {
        nevra: "vlc-3.0.24-3.fc45.x86_64".into(),
        action: Action::Remove,
        total: 2,
    });
    app.apply_progress(ProgressEvent::Done { success: true });
    assert_eq!(app.progress.success, Some(true));
    assert!(app.progress.log.iter().any(|l| l.contains("vlc")));
}

#[test]
fn troca_de_aba_mantem_selecao_valida() {
    let mut app = app_with(vec![pkg("a", Reason::User)]);
    app.tab = app.tab.step(1);
    assert_eq!(app.tab, Tab::Updates);
    assert_eq!(app.rows_len(), 0);
    assert_eq!(app.selected_index(), 0);
}

#[test]
fn busca_filtra_a_lista_visivel() {
    let mut app = app_with(vec![pkg("firefox", Reason::User), pkg("vlc", Reason::User)]);
    app.search = Some("fire".into());
    app.filter.text = "fire".into();
    app.refresh();
    assert_eq!(app.visible.len(), 1);
    assert_eq!(app.current_package().unwrap().name, "firefox");
}
