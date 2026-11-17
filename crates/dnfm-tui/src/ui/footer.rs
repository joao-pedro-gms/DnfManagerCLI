use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Tab};
use crate::theme::Theme;

pub(super) fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
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
