//! Renderização da TUI em memória: garante que o desenho não entra em pânico,
//! que a tabela respeita o viewport e que os modais aparecem.

use dnfm_core::{
    Action, HistoryEntry, Package, PlannedPackage, Preview, Reason, Repo, ResolveResult, Severity,
    Upgrade,
};
use dnfm_tui::app::{App, Modal};
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
    assert!(screen.contains("Você marcou"));
    assert!(screen.contains("Saem junto"));
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
