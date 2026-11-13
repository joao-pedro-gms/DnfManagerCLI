//! Renderização com os dados reais do daemon.
//!
//! Atravessa o caminho completo: D-Bus → modelo → taxonomia → tabela. Fica
//! atrás de `#[ignore]` porque depende do `dnf5daemon` rodando.
//!
//!     cargo test -p dnfm-tui --test real_render -- --ignored --nocapture

use dnfm_core::categories::GroupMode;
use dnfm_core::dbus::DbusBackend;
use dnfm_core::DnfBackend;
use dnfm_tui::app::{App, AppEvent};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal de teste");
    terminal
        .draw(|frame| dnfm_tui::ui::draw(frame, app))
        .expect("desenhar");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn desenha_os_pacotes_reais_agrupados_por_categoria() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let installed = backend.installed().await.expect("listar instalados");
    println!("instalados: {}", installed.len());

    let mut app = App::default();
    app.apply_event(AppEvent::Installed(installed));
    assert!(
        app.taxonomy.category_count() > 5,
        "taxonomia deveria produzir várias categorias"
    );

    app.group = GroupMode::Category;
    app.rebuild_rows();
    let screen = render(&mut app, 160, 50);
    println!(
        "categorias: {} · linhas: {}",
        app.taxonomy.category_count(),
        app.rows_len()
    );
    assert!(screen.contains("categorias"));
    assert!(screen.contains("agrupado por categoria"));

    // Os grupos vêm ordenados por tamanho — é o que aparece no topo da tela.
    let grupos = dnfm_core::categories::group_rows(
        &app.installed,
        &app.visible,
        GroupMode::Category,
        &app.taxonomy,
    );
    let maiores: Vec<String> = grupos
        .iter()
        .take(3)
        .map(|g| format!("{} ({})", g.label, g.len()))
        .collect();
    println!("maiores grupos: {}", maiores.join(", "));
    let primeiro = grupos.first().expect("ao menos um grupo").label.clone();
    assert!(
        screen.contains(&primeiro),
        "o maior grupo ({primeiro}) deveria aparecer na tela"
    );
    assert!(grupos.iter().all(|g| !g.is_empty()));

    // Filtro por categoria + render.
    let alvo = grupos[0].clone();
    app.filter.categories = vec![alvo.label.clone()];
    app.refresh();
    assert_eq!(
        app.visible.len(),
        alvo.len(),
        "o filtro deve casar a contagem do grupo"
    );
    let screen = render(&mut app, 160, 50);
    assert!(screen.contains("filtros(1)"));
}
