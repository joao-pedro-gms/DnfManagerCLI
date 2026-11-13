//! Renderização da TUI em memória: garante que o desenho não entra em pânico,
//! que a tabela respeita o viewport e que os modais aparecem.

use dnfm_core::categories::GroupMode;
use dnfm_core::{
    Action, HistoryEntry, Package, PlannedPackage, Preview, Reason, Repo, ResolveResult, Severity,
    Upgrade,
};
use dnfm_tui::app::{App, Modal, Picker, PickerTarget};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn pkg(name: &str, size: u64) -> Package {
    Package {
        name: name.into(),
        evr: "1.2.3-1.fc45".into(),
        arch: "x86_64".into(),
        repo_id: "@System".into(),
        from_repo_id: None,
        size,
        install_time: 1_793_727_256,
        summary: "um pacote de exemplo".into(),
        reason: Reason::User,
        nevra: format!("{name}-1.2.3-1.fc45.x86_64"),
        is_installed: true,
    }
}

fn populated() -> App {
    let mut app = App {
        installed: (0..200)
            .map(|i| {
                if i == 0 {
                    pkg("glibc", 12 * 1024 * 1024)
                } else {
                    pkg(&format!("pacote-{i:03}"), (i as u64) * 1024)
                }
            })
            .collect(),
        upgrades: vec![Upgrade {
            installed: pkg("firefox", 300),
            candidate: Package {
                evr: "160.0-1.fc45".into(),
                repo_id: "updates".into(),
                is_installed: false,
                ..pkg("firefox", 310)
            },
            severity: Severity::Critical,
            advisory: Some("FEDORA-2026-abc".into()),
        }],
        history: vec![HistoryEntry {
            id: 99,
            description: "dnf remove warp-terminal -y".into(),
            status: "Ok".into(),
            start: 1_794_567_129,
            end: 1_794_567_130,
            user_id: 1000,
            installed: vec![],
            removed: vec!["warp-terminal".into()],
            upgraded: vec![],
            downgraded: vec![],
        }],
        repos: vec![Repo {
            id: "fedora".into(),
            name: "Fedora 45".into(),
            enabled: true,
        }],
        ..Default::default()
    };
    app.refresh();
    app.status = "pronto".into();
    app
}

fn render(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");
    terminal
        .draw(|frame| dnfm_tui::ui::draw(frame, app))
        .expect("desenhar");
    let buffer = terminal.backend().buffer().clone();
    buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn desenha_tabela_em_terminal_pequeno() {
    let mut app = populated();
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("Instalados"));
    assert!(screen.contains("glibc"));
    assert!(screen.contains("dnfm"));
}

#[test]
fn desenha_tabela_em_terminal_grande() {
    let mut app = populated();
    let screen = render(&mut app, 200, 60);
    assert!(screen.contains("Atualizações") || screen.contains("Instalados"));
    assert!(screen.contains("detalhes"));
}

#[test]
fn nao_renderiza_mais_linhas_que_o_viewport() {
    let mut app = populated();
    app.select(150);
    let _ = render(&mut app, 100, 30);
    assert!(app.viewport > 0);
    assert!(app.scroll > 0, "deveria rolar para acompanhar a seleção");
    assert!(app.scroll <= 150);
}

#[test]
fn modal_de_preview_mostra_motivos_e_espaco() {
    let mut app = populated();
    app.modal = Some(Modal::Preview(Preview {
        items: vec![
            PlannedPackage {
                action: Action::Remove,
                reason: Reason::User,
                package: pkg("vlc", 90 * 1024 * 1024),
            },
            PlannedPackage {
                action: Action::Remove,
                reason: Reason::Dependency,
                package: pkg("SDL2_net", 1024),
            },
        ],
        problems: vec![],
        result: ResolveResult::Ok,
    }));
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("VOCÊ MARCOU"));
    assert!(screen.contains("SAEM JUNTO"));
    assert!(screen.contains("preview da transação"));
}

#[test]
fn modal_de_erro_e_ajuda_renderizam() {
    let mut app = populated();
    app.modal = Some(Modal::Help);
    assert!(render(&mut app, 100, 30).contains("ajuda"));
    app.modal = Some(Modal::Error("daemon indisponível".into()));
    assert!(render(&mut app, 100, 30).contains("daemon indisponível"));
}

#[test]
fn barra_de_status_mostra_protecao() {
    let mut app = populated();
    app.toggle_mark_current(); // glibc, protegido
    let screen = render(&mut app, 120, 30);
    assert!(screen.contains("protegido"));
}

fn app_com_categorias() -> App {
    let mut app = App {
        installed: vec![
            pkg("texlive-a2ping", 1024),
            pkg("texlive-ae", 2048),
            pkg("vlc", 4096),
            pkg("kf6-kcoreaddons", 8192),
        ],
        ..Default::default()
    };
    app.taxonomy = dnfm_core::categories::Taxonomy::build(
        &app.installed,
        &dnfm_core::categories::CustomCategories::empty(),
    );
    app.refresh();
    app
}

#[test]
fn agrupamento_por_categoria_desenha_cabecalhos() {
    let mut app = app_com_categorias();
    app.group = GroupMode::Category;
    app.rebuild_rows();
    let screen = render(&mut app, 140, 40);
    assert!(
        screen.contains("TeX Live"),
        "cabeçalho do grupo deveria aparecer"
    );
    assert!(screen.contains("2 pacote(s)"));
    assert!(screen.contains("por categoria"));
}

#[test]
fn grupo_recolhido_esconde_os_pacotes() {
    let mut app = app_com_categorias();
    app.group = GroupMode::Category;
    app.rebuild_rows();
    let antes = app.rows_len();
    app.toggle_group("TeX Live");
    let depois = app.rows_len();
    assert!(depois < antes, "recolher deve reduzir as linhas");
    let screen = render(&mut app, 140, 40);
    assert!(screen.contains("TeX Live"));
    assert!(!screen.contains("texlive-a2ping"));
}

#[test]
fn painel_de_filtros_lista_facetas() {
    let mut app = app_com_categorias();
    app.open_filter_panel();
    let screen = render(&mut app, 120, 40);
    for esperado in [
        "filtros",
        "categoria",
        "tamanho",
        "com atualização",
        "limpar tudo",
    ] {
        assert!(screen.contains(esperado), "faltou a faceta {esperado}");
    }
}

#[test]
fn seletor_mostra_contagens() {
    let mut app = app_com_categorias();
    app.open_picker(PickerTarget::Category);
    let picker: Picker = app.picker.clone().expect("picker aberto");
    assert!(picker
        .target_items
        .iter()
        .any(|(label, count)| label == "TeX Live" && *count == 2));
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("filtrar por categoria"));
    assert!(screen.contains("TeX Live"));
}

#[test]
fn campo_de_categoria_mostra_o_alvo() {
    let mut app = app_com_categorias();
    app.select(0);
    app.assign_input_start();
    app.assign_input_push('X');
    let screen = render(&mut app, 120, 40);
    assert!(screen.contains("categoria própria"));
    assert!(screen.contains("enter aplica"));
}

#[test]
fn filtro_por_categoria_reduz_a_lista() {
    let mut app = app_com_categorias();
    app.filter.categories = vec!["TeX Live".into()];
    app.refresh();
    assert_eq!(app.visible.len(), 2);
    let screen = render(&mut app, 120, 30);
    assert!(screen.contains("filtros: 1"));
}

#[test]
fn sidebar_mostra_grupos_com_contagem() {
    let mut app = app_com_categorias();
    let screen = render(&mut app, 140, 40);
    assert!(screen.contains("Categorias"), "título do sidebar");
    assert!(screen.contains("TeX Live"));
    assert!(screen.contains("grupos"), "rodapé do sidebar");
    assert!(app.sidebar_area.width > 0, "área do sidebar registrada");
}

#[test]
fn sidebar_some_em_terminal_estreito() {
    let mut app = app_com_categorias();
    let screen = render(&mut app, 70, 24);
    assert!(!screen.contains("Categorias"));
    // A tabela continua legível.
    assert!(screen.contains("Instalados"));
}

#[test]
fn chips_mostram_filtros_ativos_e_reagem_ao_clique() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    let mut app = app_com_categorias();
    app.filter.categories = vec!["TeX Live".into()];
    app.filter.only_updates = true;
    app.after_filter_change();
    let _ = render(&mut app, 140, 40);

    assert!(app.chips.len() >= 2, "um chip por filtro ativo");
    let (rect, chip) = app.chips[0].clone();
    assert!(matches!(chip, dnfm_tui::app::ChipClear::Category(_)));

    let clique = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 1,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    };
    dnfm_tui::keys::handle_mouse(&mut app, clique);
    assert!(app.filter.categories.is_empty(), "chip removeu a categoria");
    assert!(app.filter.only_updates, "o outro filtro permaneceu");
}

#[test]
fn rodape_mostra_atalhos_do_contexto() {
    let mut app = app_com_categorias();
    let screen = render(&mut app, 140, 40);
    assert!(screen.contains("marcar"));
    assert!(screen.contains("filtros"));
    assert!(screen.contains("grupos"));
}

#[test]
fn troca_de_tema_nao_quebra_o_desenho() {
    let mut app = app_com_categorias();
    for _ in 0..4 {
        let screen = render(&mut app, 120, 34);
        assert!(screen.contains("Instalados"));
        app.cycle_theme();
    }
}

#[test]
fn animacao_de_entrada_reduz_o_modal_no_primeiro_quadro() {
    let mut app = app_com_categorias();
    app.open_modal(Modal::Help);
    let inicial = render(&mut app, 120, 40);
    assert!(inicial.contains("ajuda"));
}

#[test]
fn aba_de_estatisticas_mostra_paineis() {
    use dnfm_tui::app::Tab;

    let mut app = populated();
    app.loading = false;
    app.tab = Tab::Stats;
    let screen = render(&mut app, 150, 44);

    assert!(screen.contains("RESUMO"));
    assert!(screen.contains("CATEGORIAS"));
    assert!(screen.contains("MAIORES PACOTES"));
    assert!(screen.contains("ATUALIZAÇÕES"));
    assert!(screen.contains("pacotes instalados"));
}

#[test]
fn inspetor_mostra_metadados_tags_e_arquivos() {
    let mut app = populated();
    app.loading = false;
    let pacote = app.installed[0].clone();
    app.inspection = Some(dnfm_tui::app::Inspection {
        categories: vec![dnfm_core::Tag::new(
            "Sistema",
            dnfm_core::TagSource::AppStream,
        )],
        tags: vec![
            dnfm_core::Tag::new("Sistema", dnfm_core::TagSource::AppStream),
            dnfm_core::Tag::new("biblioteca", dnfm_core::TagSource::FileSignal),
        ],
        files: Some(dnfm_core::PackageFiles {
            count: 42,
            dirs: vec![("/usr/lib64".into(), 40), ("/usr/bin".into(), 2)],
            ..Default::default()
        }),
        details: Some(dnfm_core::PackageDetails {
            nevra: pacote.nevra.clone(),
            url: "https://example.org".into(),
            license: "MIT".into(),
            description: "descrição de teste".into(),
            vendor: "Fedora Project".into(),
            group: "Unspecified".into(),
        }),
        package: pacote,
    });
    app.open_modal(Modal::Inspector);
    app.anim.advance(1.0);
    let screen = render(&mut app, 150, 44);

    assert!(screen.contains("INSPETOR"));
    assert!(screen.contains("METADADOS"));
    assert!(screen.contains("CATEGORIAS E TAGS"));
    assert!(screen.contains("ARQUIVOS"));
    assert!(screen.contains("MIT"));
    assert!(screen.contains("/usr/lib64"));
    assert!(screen.contains("42"));
}

#[test]
fn tecla_i_abre_o_inspetor() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = populated();
    app.loading = false;
    let cmds = dnfm_tui::keys::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
    );
    assert!(matches!(app.modal, Some(Modal::Inspector)));
    assert!(app.inspection.is_some());
    // Pede os detalhes que ainda faltam.
    assert!(cmds
        .iter()
        .any(|c| matches!(c, dnfm_tui::app::Cmd::LoadDetails(_))));
}
