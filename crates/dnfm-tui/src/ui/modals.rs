//! Modais e sobreposições: ajuda, erro, preview, progresso, filtros, seletor e
//! o campo de atribuição de categoria.

use dnfm_core::format_size;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, FilterFacet, Picker};

pub(crate) fn centered(area: Rect, percent_x: u16, height: u16) -> Rect {
    let height = height.min(area.height);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .areas(area);
    area
}

pub(crate) fn draw_help(frame: &mut Frame, area: Rect) {
    let rows = [
        ("espaço", "marca o pacote / recolhe o grupo"),
        ("a / u", "marca todos os visíveis / limpa marcações"),
        (
            "o / O",
            "cicla o agrupamento (categoria, família, repo, motivo)",
        ),
        ("f", "painel de filtros"),
        ("c / C", "filtra por categoria / por família"),
        ("t / T", "dá ou remove uma categoria própria"),
        ("p", "preview da remoção (marcados ou o atual)"),
        ("P", "preview da atualização selecionada"),
        ("enter", "detalhes (ou seleciona atualização)"),
        ("! / A", "seleciona atualizações de segurança / todas"),
        ("/", "busca fuzzy por nome e resumo"),
        ("s / S", "cicla ordenação / inverte"),
        ("tab", "próxima aba"),
        ("r", "recarrega a aba atual"),
        ("esc", "limpa filtros e grupos recolhidos"),
        ("q", "sai"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<8}"), Style::default().fg(Color::Cyan)),
                Span::raw(*desc),
            ])
        })
        .collect();
    let rect = centered(area, 64, (lines.len() + 4) as u16);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" ajuda — enter/esc fecha ")
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        rect,
    );
}

pub(crate) fn draw_error(frame: &mut Frame, area: Rect, message: &str) {
    let rect = centered(area, 60, 8);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(message.to_string())
            .wrap(Wrap { trim: true })
            .block(
                Block::bordered()
                    .title(" erro — enter/esc fecha ")
                    .border_style(Style::default().fg(Color::Red)),
            ),
        rect,
    );
}

pub(crate) fn draw_preview(
    frame: &mut Frame,
    area: Rect,
    preview: &dnfm_core::Preview,
    specs: &[String],
) {
    let mut lines: Vec<Line> = Vec::new();
    let requested = preview.requested();
    let collateral = preview.collateral();
    let additions: Vec<_> = preview.additions().collect();

    if preview.is_failed() && !specs.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("pedido: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(specs.join(" ")),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "Você marcou ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({})", requested.len()),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    for item in requested.iter().take(6) {
        lines.push(Line::from(format!("  − {}", item.package.name)));
    }
    if requested.len() > 6 {
        lines.push(Line::from(Span::styled(
            format!("  … e mais {}", requested.len() - 6),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(vec![
        Span::styled("Saem junto ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("({})", collateral.len()),
            Style::default().fg(Color::Red),
        ),
    ]));
    for item in collateral.iter().take(6) {
        lines.push(Line::from(vec![
            Span::raw(format!("  − {} ", item.package.name)),
            Span::styled(
                format!("({})", item.reason.label()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    if collateral.len() > 6 {
        lines.push(Line::from(Span::styled(
            format!("  … e mais {}", collateral.len() - 6),
            Style::default().fg(Color::DarkGray),
        )));
    }

    if !additions.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Entram ({})", additions.len()),
            Style::default().fg(Color::Green),
        )));
        for item in additions.iter().take(4) {
            lines.push(Line::from(format!("  + {}", item.package.name)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("espaço liberado: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_size(preview.freed_bytes()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    if !preview.problems.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "problemas de resolução:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for problem in preview.problems.iter().take(6) {
            lines.push(Line::from(Span::styled(
                format!("  {problem}"),
                Style::default().fg(Color::Red),
            )));
        }
    }

    let hint = if preview.is_failed() {
        Line::from(Span::styled(
            "resolução falhou — n/esc fecha",
            Style::default().fg(Color::Red),
        ))
    } else {
        Line::from(vec![
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" aplica · "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(" cancela"),
        ])
    };
    lines.push(hint);

    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = centered(area, 72, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::bordered()
                .title(" preview da transação ")
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        rect,
    );
}

pub(crate) fn draw_progress(frame: &mut Frame, app: &App, area: Rect) {
    let percent = if app.progress.total == 0 {
        0.0
    } else {
        (app.progress.done as f64 / app.progress.total as f64).clamp(0.0, 1.0)
    };
    let label = match app.progress.success {
        Some(true) => "concluído",
        Some(false) => "falhou",
        None => "executando…",
    };
    let mut lines = vec![Line::from(format!(
        "{} {}/{}",
        app.progress.current, app.progress.done, app.progress.total
    ))];
    for entry in app.progress.log.iter().rev().take(4).rev() {
        lines.push(Line::from(Span::styled(
            entry.clone(),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let rect = centered(area, 60, 10);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {label} "))
        .border_style(Style::default().fg(if app.progress.success == Some(false) {
            Color::Red
        } else {
            Color::Green
        }));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let [gauge_area, log_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(2)]).areas(inner);
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .ratio(percent)
            .label(format!("{:.0}%", percent * 100.0)),
        gauge_area,
    );
    frame.render_widget(Paragraph::new(lines), log_area);
}

/// Painel de filtros: uma linha por faceta, com o valor atual à direita.
pub(crate) fn draw_filter_panel(frame: &mut Frame, app: &App, area: Rect) {
    let cursor = app
        .filter_panel
        .as_ref()
        .map(|panel| panel.cursor)
        .unwrap_or(0);
    let lines: Vec<Line> = FilterFacet::ALL
        .iter()
        .enumerate()
        .map(|(idx, facet)| {
            let focused = idx == cursor;
            let marker = if focused { "▸ " } else { "  " };
            let label = format!("{marker}{:<14}", facet.label());
            let value = app.facet_value(*facet);
            let style = if focused {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(label, style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Yellow)),
            ])
        })
        .collect();

    let mut lines = lines;
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ navega · espaço/enter aplica · f/esc fecha",
        Style::default().fg(Color::DarkGray),
    )));

    let rect = centered(area, 56, (lines.len() + 2) as u16);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" filtros ")
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        rect,
    );
}

/// Seletor multi-escolha com busca.
pub(crate) fn draw_picker(frame: &mut Frame, app: &App, area: Rect, picker: &Picker) {
    let items = picker.filtered();
    let height = (area.height.saturating_sub(6)).min(24);
    let rect = centered(area, 56, height);
    frame.render_widget(Clear, rect);

    let block = Block::bordered()
        .title(format!(
            " {} — espaço marca · enter aplica ",
            picker.target.map(|t| t.title()).unwrap_or("selecionar")
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let [search_area, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("buscar: ", Style::default().fg(Color::DarkGray)),
            Span::raw(picker.query.clone()),
            Span::styled("▏", Style::default().fg(Color::Cyan)),
        ])),
        search_area,
    );

    let visible = list_area.height as usize;
    let start = picker.cursor.saturating_sub(visible.saturating_sub(1) / 2);
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(idx, (label, count))| {
            let checked = picker.selected.iter().any(|s| s == label);
            let marker = if checked { "[x]" } else { "[ ]" };
            let style = if idx == picker.cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if checked {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(format!("{label:<30}"), style),
                Span::styled(format!("{count:>5}"), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "nenhum item casa com a busca",
                Style::default().fg(Color::DarkGray),
            )),
            list_area,
        );
    } else {
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    let _ = app;
}

/// Campo de atribuição de categoria.
pub(crate) fn draw_assign(frame: &mut Frame, app: &App, area: Rect) {
    let buffer = app.assign_input.clone().unwrap_or_default();
    let target = if !app.marks.is_empty() {
        format!("{} pacote(s) marcados", app.marks.len())
    } else {
        app.current_package()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "nada selecionado".to_string())
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("categoria para ", Style::default().fg(Color::DarkGray)),
            Span::styled(target, Style::default().fg(Color::Cyan)),
            Span::styled(":", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(buffer),
            Span::styled("▏", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "enter aplica · esc cancela · nome novo cria a categoria",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let rect = centered(area, 50, lines.len() as u16 + 2);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" categoria própria ")
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        rect,
    );
}
