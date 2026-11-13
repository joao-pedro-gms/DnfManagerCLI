//! Painéis flutuantes: filtros, seletor de categoria e campo de categoria própria.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::modals::{centered, modal_block, shadow};
use crate::app::{App, FilterFacet, Picker};

pub(crate) fn draw_filter_panel(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let cursor = app
        .filter_panel
        .as_ref()
        .map(|panel| panel.cursor)
        .unwrap_or(0);

    let mut lines: Vec<Line> = Vec::new();
    for (idx, facet) in FilterFacet::ALL.iter().enumerate() {
        let em_foco = idx == cursor;
        let valor = app.facet_value(*facet);
        let marcador = if em_foco { "▸ " } else { "  " };
        let mut rotulo = format!("{marcador}{:<14}", facet.label());
        if *facet == FilterFacet::Clear {
            rotulo = format!("{marcador}{:<14}", "limpar tudo");
        }
        let style = if em_foco {
            Style::default()
                .fg(Color::Black)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        lines.push(Line::from(vec![
            Span::styled(rotulo, style),
            Span::styled(
                format!(" {valor}"),
                Style::default().fg(if em_foco { theme.accent } else { theme.text }),
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓ navega · espaço/enter aplica · f/esc fecha",
        Style::default().fg(theme.dim),
    )));

    let altura = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = centered(area, 58, altura);
    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(modal_block(
            theme,
            "filtros",
            theme.accent,
            app.anim.pulse_level(),
        )),
        rect,
    );
}

/// Seletor multi-escolha com busca.
pub(crate) fn draw_picker(frame: &mut Frame, app: &App, area: Rect, picker: &Picker) {
    let theme = &app.theme;
    let items = picker.filtered();
    let altura = (area.height.saturating_sub(6)).min(26);
    let rect = centered(area, 58, altura);
    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);

    let titulo = format!(
        "{} — espaço marca · enter aplica",
        picker.target.map(|t| t.title()).unwrap_or("selecionar")
    );
    let block = modal_block(theme, &titulo, theme.accent, app.anim.pulse_level());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let [busca, lista] = Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("buscar ", Style::default().fg(theme.dim)),
            Span::styled(picker.query.clone(), Style::default().fg(theme.text)),
            Span::styled("▏", Style::default().fg(theme.accent)),
        ])),
        busca,
    );

    let visivel = lista.height as usize;
    let inicio = picker.cursor.saturating_sub(visivel.saturating_sub(1) / 2);
    let mut linhas: Vec<Line> = Vec::new();
    for (idx, (label, count)) in items.iter().enumerate().skip(inicio).take(visivel) {
        let marcado = picker.selected.iter().any(|s| s == label);
        let em_foco = idx == picker.cursor;
        let base = if em_foco {
            Style::default().fg(Color::Black).bg(theme.accent)
        } else if marcado {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.text)
        };
        linhas.push(Line::from(vec![
            Span::styled(if marcado { "▣ " } else { "▢ " }, base),
            Span::styled(format!("{label:<30}"), base),
            Span::styled(format!("{count:>6}"), Style::default().fg(theme.dim)),
        ]));
    }

    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "nenhum item casa com a busca",
                Style::default().fg(theme.dim),
            )),
            lista,
        );
    } else {
        frame.render_widget(Paragraph::new(linhas), lista);
    }
}

/// Campo de atribuição de categoria.
pub(crate) fn draw_assign(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let buffer = app.assign_input.clone().unwrap_or_default();
    let alvo = if !app.marks.is_empty() {
        format!("{} pacote(s) marcados", app.marks.len())
    } else {
        app.current_package()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "nada selecionado".to_string())
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("categoria para ", Style::default().fg(theme.dim)),
            Span::styled(
                alvo,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(theme.accent)),
            Span::styled(buffer, Style::default().fg(theme.text)),
            Span::styled("▏", Style::default().fg(theme.accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "enter aplica · esc cancela · nome novo cria a categoria",
            Style::default().fg(theme.dim),
        )),
    ];
    let rect = centered(area, 52, lines.len() as u16 + 2);
    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(modal_block(
            theme,
            "categoria própria",
            theme.accent_alt,
            app.anim.pulse_level(),
        )),
        rect,
    );
}
