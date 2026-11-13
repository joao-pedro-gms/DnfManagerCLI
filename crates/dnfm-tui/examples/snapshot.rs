//! Gera um snapshot do layout em JSON (símbolo + cores por célula).
//!
//!     cargo run -p dnfm-tui --example snapshot -- /tmp/dnfm-ui.json [largura] [altura]
//!
//! Serve para inspeção visual e para comparar antes/depois de mexer no desenho.

use dnfm_core::{HistoryEntry, Package, Reason, Repo, Severity, Upgrade};
use dnfm_tui::app::{App, ChipClear, Modal};
use dnfm_tui::keys::cycle_group;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

fn pkg(name: &str, evr: &str, repo: &str, size: u64, reason: Reason, days_ago: i64) -> Package {
    Package {
        name: name.to_string(),
        evr: evr.to_string(),
        arch: "x86_64".to_string(),
        repo_id: repo.to_string(),
        from_repo_id: None,
        size,
        install_time: 1_793_727_256 - days_ago * 86_400,
        summary: format!("{name} — pacote de demonstração"),
        reason,
        nevra: format!("{name}-{evr}.x86_64"),
        is_installed: true,
    }
}

fn demo() -> App {
    let mut app = App::default();
    let mut pacotes = Vec::new();
    for i in 0..9 {
        pacotes.push(pkg(
            &format!("texlive-pacote{i}"),
            "12:svn15878-5.fc45",
            "@System",
            480_000 + i * 1024,
            Reason::Dependency,
            30 + i as i64,
        ));
    }
    pacotes.push(pkg(
        "kf6-kcoreaddons",
        "6.8.0-1.fc45",
        "@System",
        2_200_000,
        Reason::Dependency,
        12,
    ));
    pacotes.push(pkg(
        "kf6-kio",
        "6.8.0-1.fc45",
        "@System",
        3_100_000,
        Reason::Dependency,
        12,
    ));
    pacotes.push(pkg(
        "plasma-desktop",
        "6.8.2-1.fc45",
        "@System",
        8_400_000,
        Reason::Group,
        12,
    ));
    pacotes.push(pkg(
        "vlc",
        "1:3.0.24-3.fc45",
        "@System",
        393_855,
        Reason::User,
        10,
    ));
    pacotes.push(pkg(
        "firefox",
        "160.0-1.fc45",
        "@System",
        313_000_000,
        Reason::Group,
        1,
    ));
    pacotes.push(pkg(
        "python3-requests",
        "2.32.3-1.fc45",
        "@System",
        512_000,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "python3-pip",
        "24.2-1.fc45",
        "@System",
        7_200_000,
        Reason::User,
        40,
    ));
    pacotes.push(pkg(
        "default-fonts-core-sans",
        "4.3-3.fc45",
        "@System",
        1_100_000,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "google-noto-sans-fonts",
        "20240101-1.fc45",
        "@System",
        4_800_000,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "glibc",
        "2.42-3.fc45",
        "@System",
        12_600_000,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "systemd",
        "258-1.fc45",
        "@System",
        14_800_000,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "cockpit-bridge",
        "370-1.fc45",
        "@System",
        2_299_758,
        Reason::Dependency,
        40,
    ));
    pacotes.push(pkg(
        "docker-cli",
        "29.9.2-2.fc45",
        "@System",
        9_400_000,
        Reason::User,
        1,
    ));
    pacotes.push(pkg(
        "wine",
        "11.20-1.fc45",
        "@System",
        1_200_000,
        Reason::User,
        25,
    ));

    app.installed = pacotes;
    app.upgrades = vec![
        Upgrade {
            installed: pkg(
                "cockpit-bridge",
                "370-1.fc45",
                "@System",
                2_299_758,
                Reason::Dependency,
                40,
            ),
            candidate: Package {
                evr: "372-1.fc45".into(),
                repo_id: "updates".into(),
                is_installed: false,
                ..pkg(
                    "cockpit-bridge",
                    "372-1.fc45",
                    "updates",
                    2_310_000,
                    Reason::Dependency,
                    0,
                )
            },
            severity: Severity::Important,
            advisory: Some("FEDORA-2026-1f4c".into()),
        },
        Upgrade {
            installed: pkg(
                "firefox",
                "159.0-1.fc45",
                "@System",
                313_000_000,
                Reason::Group,
                40,
            ),
            candidate: Package {
                evr: "160.0-1.fc45".into(),
                repo_id: "updates".into(),
                is_installed: false,
                ..pkg(
                    "firefox",
                    "160.0-1.fc45",
                    "updates",
                    315_000_000,
                    Reason::Group,
                    0,
                )
            },
            severity: Severity::Critical,
            advisory: Some("FEDORA-2026-9c2a".into()),
        },
    ];
    app.history = vec![HistoryEntry {
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
    }];
    app.repos = vec![
        Repo {
            id: "fedora".into(),
            name: "Fedora 45 - x86_64".into(),
            enabled: true,
        },
        Repo {
            id: "updates".into(),
            name: "Fedora 45 - Updates".into(),
            enabled: true,
        },
    ];

    // Estado de demonstração: dois pacotes marcados e um filtro ativo.
    app.taxonomy = dnfm_core::categories::Taxonomy::build(
        &app.installed,
        &dnfm_core::categories::CustomCategories::empty(),
    );
    app.refresh();
    app.toggle_mark_current();
    app.move_selection(2);
    app.toggle_mark_current();
    app.filter.arch = Some("x86_64".into());
    app.after_filter_change();
    cycle_group(&mut app, false);
    app.select(1);
    app.loading = false;
    app.status = "2 marcados · pronto para remover".into();
    app.anim.advance(0.5);
    app
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/dnfm-ui.json".into());
    let width: u16 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(150);
    let height: u16 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(44);

    let mut app = demo();
    if args.get(4).map(|v| v == "help").unwrap_or(false) {
        app.open_modal(Modal::Help);
    }

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| dnfm_tui::ui::draw(frame, &mut app))
        .expect("desenhar");

    let buffer = terminal.backend().buffer();
    let mut rows: Vec<Vec<(String, String, String, bool)>> = Vec::new();
    for y in 0..height {
        let mut row = Vec::new();
        for x in 0..width {
            let cell = &buffer[(x, y)];
            row.push((
                cell.symbol().to_string(),
                css(cell.fg),
                css(cell.bg),
                cell.modifier.contains(ratatui::style::Modifier::BOLD),
            ));
        }
        rows.push(row);
    }
    let json = serde_json::json!({ "width": width, "height": height, "rows": rows, "chips": app.chips.len(), "groups": app.sidebar.buckets.len() });
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).expect("gravar");
    println!("snapshot em {path} ({width}x{height})");
}

fn css(color: Color) -> String {
    match color {
        Color::Reset => String::new(),
        Color::Black => "#000000".into(),
        Color::White => "#ffffff".into(),
        Color::Gray => "#b8bcc4".into(),
        Color::DarkGray => "#6e7681".into(),
        Color::Red => "#e06c75".into(),
        Color::LightRed => "#ff7b86".into(),
        Color::Green => "#98c379".into(),
        Color::LightGreen => "#b5e890".into(),
        Color::Yellow => "#e5c07b".into(),
        Color::LightYellow => "#f5d79b".into(),
        Color::Blue => "#61afef".into(),
        Color::LightBlue => "#8fd0ff".into(),
        Color::Magenta => "#c678dd".into(),
        Color::LightMagenta => "#e0a3f5".into(),
        Color::Cyan => "#56b6c2".into(),
        Color::LightCyan => "#8fdde8".into(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => format!("#{:02x}{:02x}{:02x}", i, i, i),
    }
}

#[allow(dead_code)]
fn _chip(_: ChipClear) {}
