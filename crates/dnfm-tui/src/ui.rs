//! Desenho da interface. Só lê o estado; nenhuma decisão vive aqui.

use dnfm_core::{format_date, format_size, Reason};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{App, Modal, RowRef, Tab};

mod modals;

pub(crate) const HINT: &str =
    "␣ marca · t categoria · f filtros · o agrupa · p preview · / busca · s ordena · ? ajuda · q sai";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, body, details, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(7),
        Constraint::Length(2),
    ])
    .areas(frame.area());

    draw_header(frame, app, header);
    draw_table(frame, app, body);
    draw_details(frame, app, details);
    draw_footer(frame, app, footer);

    // Sobreposições, da mais específica para a mais genérica.
    if app.assign_input.is_some() {
        modals::draw_assign(frame, app, frame.area());
    }
    if let Some(picker) = app.picker.clone() {
        modals::draw_picker(frame, app, frame.area(), &picker);
    } else if app.filter_panel.is_some() {
        modals::draw_filter_panel(frame, app, frame.area());
    }
    if let Some(modal) = app.modal.clone() {
        match modal {
            Modal::Help => modals::draw_help(frame, frame.area()),
            Modal::Error(message) => modals::draw_error(frame, frame.area(), &message),
            Modal::Preview(preview) => {
                modals::draw_preview(frame, frame.area(), &preview, &app.pending_specs)
            }
            Modal::Progress => modals::draw_progress(frame, app, frame.area()),
        }
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let marked = app.marks.len();
    let active = app.filter.active_count();
    let filtro = if active > 0 {
        format!(" · {active} filtro(s)")
    } else {
        String::new()
    };
    let title = format!(
        " dnfm · {} instalados · {} atualizações · {} marcados · {} categorias{filtro} ",
        app.installed.len(),
        app.upgrades.len(),
        marked,
        app.taxonomy.category_count()
    );
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| Line::from(t.title().to_string()))
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ")
        .block(
            Block::bordered()
                .title(title)
                .title_alignment(Alignment::Left)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(tabs, area);
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = match app.tab {
        Tab::Installed if app.group != dnfm_core::categories::GroupMode::None => {
            format!(" {} · agrupado por {} ", app.tab.title(), app.group.label())
        }
        _ => format!(" {} ", app.tab.title()),
    };
    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.table_area = inner;
    app.viewport = inner.height.saturating_sub(1) as usize;
    app.follow_selection();

    let (header, widths) = table_layout(app.tab);
    let end = (app.scroll + app.viewport).min(app.rows_len());
    let rows: Vec<Row> = (app.scroll..end).map(|idx| build_row(app, idx)).collect();

    let mut state = TableState::default();
    let selected = app.selected_index().saturating_sub(app.scroll);
    if !rows.is_empty() {
        state.select(Some(selected.min(rows.len() - 1)));
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::Indexed(236))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(table, inner, &mut state);
}

fn table_layout(tab: Tab) -> (Vec<&'static str>, Vec<Constraint>) {
    match tab {
        Tab::Installed | Tab::Orphans => (
            vec![
                " ",
                "nome",
                "versão",
                "repo",
                "tamanho",
                "instalado",
                "categoria",
                "motivo",
            ],
            vec![
                Constraint::Length(4),
                Constraint::Length(30),
                Constraint::Length(22),
                Constraint::Length(11),
                Constraint::Length(10),
                Constraint::Length(11),
                Constraint::Length(18),
                Constraint::Min(9),
            ],
        ),
        Tab::Updates => (
            vec![
                " ",
                "nome",
                "instalado",
                "disponível",
                "severidade",
                "advisory",
            ],
            vec![
                Constraint::Length(4),
                Constraint::Length(30),
                Constraint::Length(20),
                Constraint::Length(20),
                Constraint::Length(12),
                Constraint::Min(12),
            ],
        ),
        Tab::History => (
            vec!["id", "data", "status", "mudanças", "comando"],
            vec![
                Constraint::Length(6),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(20),
            ],
        ),
        Tab::Repos => (
            vec!["id", "nome", "estado"],
            vec![
                Constraint::Length(40),
                Constraint::Min(20),
                Constraint::Length(10),
            ],
        ),
    }
}

fn build_row(app: &App, idx: usize) -> Row<'static> {
    match app.tab {
        Tab::Installed => match app.rows.get(idx) {
            Some(RowRef::Header {
                label,
                count,
                collapsed,
            }) => {
                let marker = if *collapsed { "▶" } else { "▼" };
                let mut cells = vec![
                    Cell::from(marker).style(Style::default().fg(Color::Cyan)),
                    Cell::from(label.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(format!("{count} pacote(s)"))
                        .style(Style::default().fg(Color::DarkGray)),
                ];
                while cells.len() < 8 {
                    cells.push(Cell::from(""));
                }
                Row::new(cells)
            }
            Some(RowRef::Package { index }) => match app.installed.get(*index) {
                Some(pkg) => package_row(app, pkg),
                None => Row::new(Vec::<Cell>::new()),
            },
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Orphans => match app.orphans.get(idx) {
            Some(pkg) => package_row(app, pkg),
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Updates => match app.upgrades.get(idx) {
            Some(up) => {
                let checked = if app.selected_upgrades.contains(&up.installed.name) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let severity_style = match up.severity {
                    dnfm_core::Severity::Critical => {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    }
                    dnfm_core::Severity::Important => Style::default().fg(Color::LightRed),
                    dnfm_core::Severity::Moderate => Style::default().fg(Color::Yellow),
                    dnfm_core::Severity::Low => Style::default().fg(Color::Blue),
                    dnfm_core::Severity::Unknown => Style::default().fg(Color::DarkGray),
                };
                Row::new(vec![
                    Cell::from(checked),
                    Cell::from(up.installed.name.clone()),
                    Cell::from(up.installed.evr.clone())
                        .style(Style::default().fg(Color::DarkGray)),
                    Cell::from(up.candidate.evr.clone()).style(Style::default().fg(Color::Green)),
                    Cell::from(up.severity.label()).style(severity_style),
                    Cell::from(up.advisory.clone().unwrap_or_else(|| "—".into())),
                ])
            }
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::History => match app.history.get(idx) {
            Some(entry) => Row::new(vec![
                Cell::from(entry.id.to_string()),
                Cell::from(format_date(entry.start)),
                Cell::from(entry.status.clone()).style(if entry.status == "Ok" {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                }),
                Cell::from(entry.changes().to_string()),
                Cell::from(entry.description.clone()),
            ]),
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Repos => match app.repos.get(idx) {
            Some(repo) => Row::new(vec![
                Cell::from(repo.id.clone()),
                Cell::from(repo.name.clone()),
                Cell::from(if repo.enabled { "ativo" } else { "inativo" }).style(if repo.enabled {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
            ]),
            None => Row::new(Vec::<Cell>::new()),
        },
    }
}

fn package_row(app: &App, pkg: &dnfm_core::Package) -> Row<'static> {
    let protected = dnfm_core::safety::is_protected(&pkg.name).is_some();
    let checked = if app.is_marked(pkg) {
        Span::styled("[x]", Style::default().fg(Color::Yellow))
    } else if protected {
        Span::styled("[!]", Style::default().fg(Color::Red))
    } else {
        Span::raw("[ ]")
    };
    let name_style = if protected {
        Style::default().fg(Color::Red)
    } else if app.is_marked(pkg) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let category = app.category_label(pkg);
    let category_style = if app.custom_categories.get(&pkg.name, &pkg.arch).is_some() {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::Gray)
    };
    Row::new(vec![
        Cell::from(checked),
        Cell::from(pkg.name.clone()).style(name_style),
        Cell::from(pkg.evr.clone()),
        Cell::from(pkg.repo_id.clone()).style(Style::default().fg(Color::DarkGray)),
        Cell::from(format_size(pkg.size)),
        Cell::from(format_date(pkg.install_time)).style(Style::default().fg(Color::DarkGray)),
        Cell::from(category.to_string()).style(category_style),
        Cell::from(pkg.reason.label()).style(match pkg.reason {
            Reason::User => Style::default().fg(Color::Cyan),
            _ => Style::default().fg(Color::DarkGray),
        }),
    ])
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered()
        .title(" detalhes ")
        .border_style(Style::default().fg(Color::DarkGray));
    let mut lines: Vec<Line> = Vec::new();

    if let Some(pkg) = app.current_package() {
        lines.push(Line::from(vec![
            Span::styled(
                pkg.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(pkg.evr.clone(), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(pkg.arch.clone(), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", app.category_label(pkg)),
                Style::default().fg(Color::Cyan),
            ),
        ]));
        lines.push(Line::from(pkg.summary.clone()));
        if let Some(details) = &app.details {
            if !details.description.is_empty() {
                let desc: String = details
                    .description
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(Line::from(Span::styled(
                    desc,
                    Style::default().fg(Color::Gray),
                )));
            }
            let mut meta = Vec::new();
            if !details.license.is_empty() {
                meta.push(format!("licença: {}", details.license));
            }
            if !details.vendor.is_empty() {
                meta.push(format!("vendor: {}", details.vendor));
            }
            if !meta.is_empty() {
                lines.push(Line::from(Span::styled(
                    meta.join("   "),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            if !details.url.is_empty() {
                lines.push(Line::from(Span::styled(
                    details.url.clone(),
                    Style::default().fg(Color::Blue),
                )));
            }
        }
        if let Some(mark) = app.marks.get(&pkg.name, &pkg.arch) {
            lines.push(Line::from(Span::styled(
                format!("marcado para remoção em {}", format_date(mark.marked_at)),
                Style::default().fg(Color::Yellow),
            )));
        }
        if let Some(reason) = dnfm_core::safety::is_protected(&pkg.name) {
            lines.push(Line::from(Span::styled(
                format!("protegido: {reason}"),
                Style::default().fg(Color::Red),
            )));
        }
    } else if let Some(up) = app.current_upgrade() {
        lines.push(Line::from(vec![
            Span::styled(
                up.installed.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                up.installed.evr.clone(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" → "),
            Span::styled(up.candidate.evr.clone(), Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(up.candidate.summary.clone()));
        if let Some(advisory) = &up.advisory {
            lines.push(Line::from(Span::styled(
                format!("advisory {advisory} · {}", up.severity.label()),
                Style::default().fg(Color::Yellow),
            )));
        }
    } else if let Some(RowRef::Header { label, count, .. }) = app.current_row() {
        lines.push(Line::from(vec![
            Span::styled(
                label.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  {count} pacote(s)")),
        ]));
        lines.push(Line::from(Span::styled(
            "espaço recolhe/expande o grupo · a marca o grupo inteiro",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "nada selecionado",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status_style = if app.loading {
        Style::default().fg(Color::Yellow)
    } else if app.status.contains("protegido") || app.status.contains("falhou") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Gray)
    };

    let left = if let Some(buffer) = &app.search {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(buffer.clone()),
            Span::styled("▏", Style::default().fg(Color::Cyan)),
            Span::styled(
                "  (enter aplica · esc limpa)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let filtros = if app.filter.is_active() {
            format!("filtros({})", app.filter.active_count())
        } else {
            "sem filtro".to_string()
        };
        Line::from(vec![
            Span::styled(app.status.clone(), status_style),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", app.sort.label()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", app.group.label()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(filtros, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(app.selection_label(), Style::default().fg(Color::DarkGray)),
        ])
    };
    let hints = Line::from(Span::styled(HINT, Style::default().fg(Color::DarkGray)));
    frame.render_widget(Paragraph::new(vec![left, hints]), area);
}
