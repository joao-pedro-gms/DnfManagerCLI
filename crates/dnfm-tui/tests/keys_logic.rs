//! Testes de teclado: cada tecla vira o comando esperado.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dnfm_core::categories::GroupMode;
use dnfm_core::{Package, Reason};
use dnfm_tui::app::{App, Cmd, Modal, Tab};
use dnfm_tui::keys::handle_key;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app() -> App {
    let mut app = App {
        installed: vec![Package {
            name: "vlc".into(),
            evr: "3.0.24-3.fc45".into(),
            arch: "x86_64".into(),
            repo_id: "@System".into(),
            from_repo_id: None,
            size: 1024,
            install_time: 0,
            summary: String::new(),
            reason: Reason::User,
            nevra: "vlc-3.0.24-3.fc45.x86_64".into(),
            is_installed: true,
        }],
        ..Default::default()
    };
    app.rebuild_taxonomy();
    app
}

#[test]
fn espaco_marca_pacote_sob_o_cursor() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(app.marks.len(), 1);
}

#[test]
fn barra_abre_busca_e_filtra_ao_digitar() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('/')));
    assert!(app.search.is_some());
    handle_key(&mut app, key(KeyCode::Char('v')));
    assert_eq!(app.visible.len(), 1);
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.search.is_none());
    assert!(app.filter.text.is_empty());
}

#[test]
fn q_sai_mas_esc_so_limpa_filtro() {
    let mut app = app();
    assert!(handle_key(&mut app, key(KeyCode::Esc)).is_empty());
    assert!(!app.quit);
    let cmds = handle_key(&mut app, key(KeyCode::Char('q')));
    assert!(matches!(cmds.as_slice(), [Cmd::Quit]));
    assert!(app.quit);
}

#[test]
fn p_gera_preview_com_a_marcacao() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char(' ')));
    let cmds = handle_key(&mut app, key(KeyCode::Char('p')));
    assert!(matches!(cmds.as_slice(), [Cmd::Preview(_)]));
}

#[test]
fn tab_troca_de_aba() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.tab, Tab::Updates);
}

#[test]
fn ajuda_abre_e_fecha() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('?')));
    assert!(matches!(app.modal, Some(Modal::Help)));
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.modal.is_none());
}

#[test]
fn o_cicla_o_agrupamento() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('o')));
    assert_eq!(app.group, GroupMode::Category);
    assert!(app.status.contains("agrupado"));
    handle_key(&mut app, key(KeyCode::Char('O')));
    assert_ne!(app.group, GroupMode::Category);
}

#[test]
fn f_abre_e_fecha_o_painel_de_filtros() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('f')));
    assert!(app.filter_panel.is_some());
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.filter_panel.is_none());
}

#[test]
fn enter_na_faceta_de_texto_abre_a_busca() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('f')));
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.filter_panel.is_none());
    assert!(app.search.is_some());
}

#[test]
fn c_abre_o_seletor_e_enter_aplica() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('c')));
    assert!(app.picker.is_some());
    handle_key(&mut app, key(KeyCode::Char(' ')));
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.picker.is_none());
}

#[test]
fn t_atribui_categoria_ao_pacote_atual() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('t')));
    assert!(app.assign_input.is_some());
    for ch in "Meus apps".chars() {
        handle_key(&mut app, key(KeyCode::Char(ch)));
    }
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.assign_input.is_none());
    assert_eq!(app.category_label(&app.installed[0]), "Meus apps");
}

#[test]
fn esc_no_campo_de_categoria_cancela() {
    let mut app = app();
    handle_key(&mut app, key(KeyCode::Char('t')));
    handle_key(&mut app, key(KeyCode::Char('X')));
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.assign_input.is_none());
    assert_ne!(app.category_label(&app.installed[0]), "X");
}

#[test]
fn espaco_em_cabecalho_recolhe_o_grupo() {
    let mut app = app();
    app.group = GroupMode::Category;
    app.rebuild_rows();
    // Cursor direto no cabeçalho (a navegação normal pularia para o pacote).
    app.table.select(Some(0));
    assert!(app.current_row().map(|r| r.is_header()).unwrap_or(false));
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(app.collapsed.len(), 1);
    assert_eq!(app.marks.len(), 0);
}
