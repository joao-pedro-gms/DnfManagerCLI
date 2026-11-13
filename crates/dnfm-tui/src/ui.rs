//! Desenho da interface: layout, tabela, detalhes e rodapé.
//!
//! Só lê o estado; nenhuma decisão vive aqui. O tema e o animador vêm do `App`,
//! então a mesma função desenha qualquer paleta em qualquer quadro.

use dnfm_core::{format_date, format_size, Reason};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::{App, ChipClear, Modal, RowRef, Tab};
use crate::theme::Theme;

mod header;
mod inspector;
mod modals;
mod panels;
mod sidebar;
mod stats_view;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let [topo, corpo, detalhes, rodape] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(8),
        Constraint::Length(5),
        Constraint::Length(2),
    ])
    .areas(area);

    header::draw(frame, app, topo);

    // O sidebar só faz sentido na aba de instalados e com largura suficiente.
    let com_sidebar = app.sidebar.visible && app.tab == Tab::Installed && area.width >= 92;
    if com_sidebar {
        let [lado, resto] =
            Layout::horizontal([Constraint::Length(32), Constraint::Min(40)]).areas(corpo);
        sidebar::draw(frame, app, lado);
        draw_table(frame, app, resto);
    } else {
        app.sidebar_area = Rect::default();
        if app.tab == Tab::Stats {
            stats_view::draw(frame, app, corpo);
        } else {
            draw_table(frame, app, corpo);
        }
    }

    draw_details(frame, app, detalhes);
    draw_footer(frame, app, rodape);

    // Sobreposições: campo de categoria, seletor/painel de filtros, modal.
    if app.assign_input.is_some() {
        panels::draw_assign(frame, app, area);
    }
    if let Some(picker) = app.picker.clone() {
        panels::draw_picker(frame, app, area, &picker);
    } else if app.filter_panel.is_some() {
        panels::draw_filter_panel(frame, app, area);
    }
    if let Some(modal) = app.modal.clone() {
        let rect = modals::modal_rect(frame.area(), modal_geometry(&modal), app.anim.modal_eased());
        match modal {
            Modal::Help => modals::draw_help(frame, app, rect),
            Modal::Error(message) => modals::draw_error(frame, app, rect, &message),
            Modal::Preview(preview) => {
                modals::draw_preview(frame, app, rect, &preview, &app.pending_specs)
            }
            Modal::Progress => modals::draw_progress(frame, app, rect),
            Modal::Inspector => inspector::draw(frame, app, rect),
        }
    }
}

/// Proporção (largura, altura) de cada modal.
fn modal_geometry(modal: &Modal) -> (u16, u16) {
    match modal {
        Modal::Help => (66, 22),
        Modal::Error(_) => (60, 10),
        Modal::Preview(_) => (76, 30),
        Modal::Progress => (62, 12),
        Modal::Inspector => (92, 36),
    }
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.clone();
    let filtrados = app.visible.len();
    let total = app.installed.len();
    let titulo = match app.tab {
        Tab::Installed => {
            let agrup = if app.group != dnfm_core::categories::GroupMode::None {
                format!(" · por {}", app.group.label())
            } else {
                String::new()
            };
            if filtrados == total {
                format!(" {} · {total}{agrup} ", app.tab.title())
            } else {
                format!(" {} · {filtrados} de {total}{agrup} ", app.tab.title())
            }
        }
        _ => format!(" {} ", app.tab.title()),
    };
    let block = Block::bordered()
        .title(Span::styled(titulo, Style::default().fg(theme.text)))
        .border_type(BorderType::Rounded)
        .border_style(theme.border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.table_area = inner;
    app.viewport = inner.height.saturating_sub(1) as usize;
    app.follow_selection();

    let (headers, widths) = table_layout(app.tab);
    let end = (app.scroll + app.viewport).min(app.rows_len());
    let rows: Vec<Row> = (app.scroll..end)
        .map(|idx| build_row(app, idx, &theme))
        .collect();

    let mut state = TableState::default();
    let selected = app.selected_index().saturating_sub(app.scroll);
    if !rows.is_empty() {
        state.select(Some(selected.min(rows.len() - 1)));
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(headers).style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .row_highlight_style(theme.row_highlight())
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
                "  tamanho",
                "instalado",
                "categoria",
                "motivo",
            ],
            vec![
                Constraint::Length(3),
                Constraint::Length(26),
                Constraint::Length(20),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(10),
                Constraint::Length(16),
                Constraint::Min(11),
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
                Constraint::Length(28),
                Constraint::Length(19),
                Constraint::Length(19),
                Constraint::Length(11),
                Constraint::Min(12),
            ],
        ),
        Tab::History => (
            vec!["id", "data", "status", "mudanças", "comando"],
            vec![
                Constraint::Length(6),
                Constraint::Length(11),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Min(20),
            ],
        ),
        Tab::Stats => (vec![" ", "métrica", "valor"], vec![Constraint::Min(20)]),
        Tab::Repos => (
            vec!["id", "nome", "estado"],
            vec![
                Constraint::Length(38),
                Constraint::Min(18),
                Constraint::Length(9),
            ],
        ),
    }
}

fn build_row(app: &App, idx: usize, theme: &Theme) -> Row<'static> {
    let zebra = Style::default().bg(theme.surface);
    let hover = Style::default().bg(theme.surface_alt);
    let fundo = if app.hover == Some(idx) {
        hover
    } else if idx % 2 == 1 {
        zebra
    } else {
        Style::default()
    };

    match app.tab {
        Tab::Stats => Row::new(vec![Cell::from("")]),
        Tab::Installed => match app.rows.get(idx) {
            Some(RowRef::Header {
                label,
                count,
                collapsed,
            }) => {
                let pct = if app.visible.is_empty() {
                    0
                } else {
                    *count * 100 / app.visible.len().max(1)
                };
                let barra_largura = 12usize;
                let preenchido = ((*count as f32 / app.visible.len().max(1) as f32)
                    * barra_largura as f32)
                    .round() as usize;
                let barra = format!(
                    "{}{}",
                    "█".repeat(preenchido.min(barra_largura)),
                    "·".repeat(barra_largura.saturating_sub(preenchido))
                );
                let mut cells = vec![
                    Cell::from(if *collapsed { "▶" } else { "▼" })
                        .style(Style::default().fg(theme.accent)),
                    Cell::from(label.clone()).style(
                        Style::default()
                            .fg(theme.pulsed(app.anim.pulse_level()))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(format!("{count} · {pct}%")).style(Style::default().fg(theme.dim)),
                    Cell::from("").style(zebra),
                    Cell::from("").style(zebra),
                    Cell::from("").style(zebra),
                    Cell::from(barra).style(Style::default().fg(theme.accent)),
                    Cell::from(if *collapsed { "recolhido" } else { "" })
                        .style(Style::default().fg(theme.dim)),
                ];
                while cells.len() < 8 {
                    cells.push(Cell::from(""));
                }
                Row::new(cells).style(Style::default().bg(theme.surface_alt))
            }
            Some(RowRef::Package { index }) => match app.installed.get(*index) {
                Some(pkg) => package_row(app, pkg, theme).style(fundo),
                None => Row::new(Vec::<Cell>::new()),
            },
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Orphans => match app.orphans.get(idx) {
            Some(pkg) => package_row(app, pkg, theme).style(fundo),
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Updates => match app.upgrades.get(idx) {
            Some(up) => {
                let checked = if app.selected_upgrades.contains(&up.installed.name) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let sev = match up.severity {
                    dnfm_core::Severity::Critical => Style::default()
                        .fg(theme.danger)
                        .add_modifier(Modifier::BOLD),
                    dnfm_core::Severity::Important => Style::default().fg(theme.danger),
                    dnfm_core::Severity::Moderate => Style::default().fg(theme.warn),
                    dnfm_core::Severity::Low => Style::default().fg(theme.accent),
                    dnfm_core::Severity::Unknown => Style::default().fg(theme.dim),
                };
                Row::new(vec![
                    Cell::from(checked).style(Style::default().fg(theme.mark)),
                    Cell::from(up.installed.name.clone()).style(Style::default().fg(theme.text)),
                    Cell::from(up.installed.evr.clone()).style(Style::default().fg(theme.dim)),
                    Cell::from(up.candidate.evr.clone()).style(Style::default().fg(theme.ok)),
                    Cell::from(up.severity.label()).style(sev),
                    Cell::from(up.advisory.clone().unwrap_or_else(|| "—".into()))
                        .style(Style::default().fg(theme.dim)),
                ])
                .style(fundo)
            }
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::History => match app.history.get(idx) {
            Some(entry) => Row::new(vec![
                Cell::from(entry.id.to_string()).style(Style::default().fg(theme.dim)),
                Cell::from(format_date(entry.start)),
                Cell::from(entry.status.clone()).style(if entry.status == "Ok" {
                    Style::default().fg(theme.ok)
                } else {
                    Style::default().fg(theme.danger)
                }),
                Cell::from(entry.changes().to_string()),
                Cell::from(entry.description.clone()),
            ])
            .style(fundo),
            None => Row::new(Vec::<Cell>::new()),
        },
        Tab::Repos => match app.repos.get(idx) {
            Some(repo) => Row::new(vec![
                Cell::from(repo.id.clone()),
                Cell::from(repo.name.clone()),
                Cell::from(if repo.enabled { "ativo" } else { "inativo" }).style(if repo.enabled {
                    Style::default().fg(theme.ok)
                } else {
                    Style::default().fg(theme.dim)
                }),
            ])
            .style(fundo),
            None => Row::new(Vec::<Cell>::new()),
        },
    }
}

fn package_row(app: &App, pkg: &dnfm_core::Package, theme: &Theme) -> Row<'static> {
    let protected = dnfm_core::safety::is_protected(&pkg.name).is_some();
    let marcado = app.is_marked(pkg);
    let checked = if marcado {
        Span::styled("▣", Style::default().fg(theme.mark))
    } else if protected {
        Span::styled("▨", Style::default().fg(theme.danger))
    } else {
        Span::styled("▢", Style::default().fg(theme.dim))
    };
    let nome_style = if protected {
        Style::default().fg(theme.danger)
    } else if marcado {
        Style::default().fg(theme.mark)
    } else {
        Style::default().fg(theme.text)
    };
    let propria = app.custom_categories.get(&pkg.name, &pkg.arch).is_some();
    let categoria = app.category_label(pkg);
    Row::new(vec![
        Cell::from(checked),
        Cell::from(pkg.name.clone()).style(nome_style),
        Cell::from(pkg.evr.clone()).style(Style::default().fg(theme.text)),
        Cell::from(pkg.repo_id.clone()).style(Style::default().fg(theme.dim)),
        Cell::from(format!("{:>9}", format_size(pkg.size))).style(Style::default().fg(theme.dim)),
        Cell::from(format!("{:>10}", format_date(pkg.install_time)))
            .style(Style::default().fg(theme.dim)),
        Cell::from(categoria.to_string()).style(if propria {
            Style::default().fg(theme.accent_alt)
        } else {
            Style::default().fg(theme.text)
        }),
        Cell::from(pkg.reason.label()).style(match pkg.reason {
            Reason::User => Style::default().fg(theme.accent),
            _ => Style::default().fg(theme.dim),
        }),
    ])
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme.clone();
    let block = Block::bordered()
        .title(Span::styled(" detalhes ", Style::default().fg(theme.dim)))
        .border_type(BorderType::Rounded)
        .border_style(theme.border());
    let mut lines: Vec<Line> = Vec::new();

    if let Some(pkg) = app.current_package() {
        lines.push(Line::from(vec![
            Span::styled(
                pkg.name.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {}", pkg.evr), Style::default().fg(theme.dim)),
            Span::styled(format!("  ·  {}", pkg.arch), Style::default().fg(theme.dim)),
        ]));
        lines.push(Line::from(vec![Span::styled(
            pkg.summary.clone(),
            Style::default().fg(theme.text),
        )]));
        if app.details.is_none() {
            lines.push(Line::from(Span::styled(
                "enter: descrição, licença e URL",
                Style::default().fg(theme.dim),
            )));
        }
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
                    Style::default().fg(theme.dim),
                )));
            }
            let mut meta: Vec<Span> = Vec::new();
            if !details.license.is_empty() {
                meta.push(Span::styled(
                    format!("licença {}", details.license),
                    Style::default().fg(theme.dim),
                ));
            }
            if !details.vendor.is_empty() {
                meta.push(Span::styled(
                    format!("  ·  {}", details.vendor),
                    Style::default().fg(theme.dim),
                ));
            }
            if !details.url.is_empty() {
                meta.push(Span::styled(
                    format!("  ·  {}", details.url),
                    Style::default().fg(theme.accent),
                ));
            }
            if !meta.is_empty() {
                lines.push(Line::from(meta));
            }
        }
        let mut avisos: Vec<Span> = Vec::new();
        if let Some(mark) = app.marks.get(&pkg.name, &pkg.arch) {
            avisos.push(Span::styled(
                format!("marcado em {}", format_date(mark.marked_at)),
                Style::default().fg(theme.mark),
            ));
        }
        if let Some(reason) = dnfm_core::safety::is_protected(&pkg.name) {
            avisos.push(Span::styled(
                format!("  protegido: {reason}"),
                Style::default().fg(theme.danger),
            ));
        }
        if let Some(propria) = app.custom_categories.get(&pkg.name, &pkg.arch) {
            avisos.push(Span::styled(
                format!("  categoria própria: {propria}"),
                Style::default().fg(theme.accent_alt),
            ));
        }
        if !avisos.is_empty() {
            lines.push(Line::from(avisos));
        }
    } else if let Some(up) = app.current_upgrade() {
        lines.push(Line::from(vec![
            Span::styled(
                up.installed.name.clone(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} → ", up.installed.evr),
                Style::default().fg(theme.dim),
            ),
            Span::styled(up.candidate.evr.clone(), Style::default().fg(theme.ok)),
        ]));
        lines.push(Line::from(up.candidate.summary.clone()));
        if let Some(advisory) = &up.advisory {
            lines.push(Line::from(Span::styled(
                format!("advisory {advisory} · {}", up.severity.label()),
                Style::default().fg(theme.warn),
            )));
        }
    } else if let Some(RowRef::Header { label, count, .. }) = app.current_row() {
        lines.push(Line::from(vec![
            Span::styled(
                label.clone(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {count} pacote(s)"),
                Style::default().fg(theme.dim),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            "espaço recolhe o grupo · a marca o grupo inteiro · clique no sidebar filtra",
            Style::default().fg(theme.dim),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "nada selecionado",
            Style::default().fg(theme.dim),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme.clone();
    let status_style = if app.loading {
        Style::default().fg(theme.warn)
    } else if app.status.contains("protegido") || app.status.contains("falhou") {
        Style::default().fg(theme.danger)
    } else {
        Style::default().fg(theme.text)
    };

    let esquerda = if let Some(buffer) = &app.search {
        Line::from(vec![
            Span::styled("buscar ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("/{buffer}"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("▏", Style::default().fg(theme.accent)),
            Span::styled("  enter aplica · esc limpa", Style::default().fg(theme.dim)),
        ])
    } else {
        let mut spans = vec![
            Span::styled(
                if app.loading {
                    format!("{} ", app.anim.spinner())
                } else {
                    "● ".to_string()
                },
                Style::default().fg(if app.loading { theme.warn } else { theme.ok }),
            ),
            Span::styled(app.status.clone(), status_style),
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("ordenar: {}", app.sort.label()),
                Style::default().fg(theme.dim),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("grupo: {}", app.group.label()),
                Style::default().fg(theme.dim),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("filtros: {}", app.filter.active_count()),
                Style::default().fg(if app.filter.is_active() {
                    theme.accent
                } else {
                    theme.dim
                }),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.dim)),
            Span::styled(app.selection_label(), Style::default().fg(theme.dim)),
        ];
        if app.loading {
            spans.push(Span::styled(
                format!("  {}", app.anim.spinner()),
                Style::default().fg(theme.warn),
            ));
        }
        Line::from(spans)
    };

    frame.render_widget(Paragraph::new(vec![esquerda, keybar(app, &theme)]), area);
}

fn keybar<'a>(app: &App, theme: &Theme) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut push = |key: &'a str, desc: &'a str| {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(theme.text)
                .bg(theme.surface_alt)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {desc}  "),
            Style::default().fg(theme.dim),
        ));
    };

    match app.tab {
        Tab::Installed | Tab::Orphans => {
            push("␣", "marcar");
            push("t", "categoria");
            push("f", "filtros");
            push("o", "agrupar");
            push("[ ]", "grupos");
            push("p", "preview");
            push("/", "buscar");
        }
        Tab::Updates => {
            push("␣", "selecionar");
            push("!", "só segurança");
            push("P", "atualizar");
            push("r", "recarregar");
        }
        Tab::History => {
            push("r", "recarregar");
            push("tab", "abas");
        }
        Tab::Repos => {
            push("r", "recarregar");
            push("tab", "abas");
        }
        Tab::Stats => {
            push("r", "recalcular");
            push("tab", "abas");
        }
    }
    push("?", "ajuda");
    push("q", "sair");
    Line::from(spans)
}

/// Chips visíveis, exportado para os testes conferirem a barra de filtros.
pub fn active_chips(app: &App) -> Vec<(String, ChipClear)> {
    header::chip_labels(&app.filter)
}
