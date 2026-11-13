//! Modais (ajuda, erro, preview, progresso) e o utilitário de centralização.

use dnfm_core::format_size;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub(crate) fn centered(area: Rect, percent_x: u16, height: u16) -> Rect {
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
        ("espaço", "marca/desmarca o pacote sob o cursor"),
        ("a / u", "marca todos os visíveis / limpa marcações"),
        ("p", "preview da remoção (marcados ou o atual)"),
        ("P", "preview da atualização selecionada"),
        ("enter", "detalhes (ou seleciona atualização)"),
        ("! / A", "seleciona atualizações de segurança / todas"),
        ("/", "busca fuzzy por nome e resumo"),
        ("s / S", "cicla ordenação / inverte"),
        ("tab", "próxima aba"),
        ("r", "recarrega a aba atual"),
        ("esc", "limpa filtros / fecha modal"),
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
    let rect = centered(area, 60, (lines.len() + 4) as u16);
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
