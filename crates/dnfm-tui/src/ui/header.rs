//! Cabeçalho: marca, contadores, abas e chips de filtro clicáveis.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::{App, ChipClear, Tab};

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.clone();
    let block = Block::bordered()
        .border_style(theme.border())
        .border_type(ratatui::widgets::BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Linha 1: marca + contadores (e spinner enquanto carrega).
    let mut titulo = vec![
        Span::styled(
            "dnfm",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} pacotes", app.installed.len()),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            format!("  ·  {} atualizações", app.upgrades.len()),
            Style::default().fg(theme.dim),
        ),
        Span::styled(
            format!("  ·  {} marcados", app.marks.len()),
            Style::default().fg(if !app.marks.is_empty() {
                theme.mark
            } else {
                theme.dim
            }),
        ),
        Span::styled(
            format!("  ·  {} categorias", app.taxonomy.category_count()),
            Style::default().fg(theme.dim),
        ),
    ];
    if app.loading {
        titulo.push(Span::styled(
            format!("  {} resolvendo…", app.anim.spinner()),
            Style::default().fg(theme.warn),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(titulo)),
        Rect { height: 1, ..inner },
    );

    // Linha 2: abas + chips de filtro.
    let linha_y = inner.y + 1;
    if inner.height >= 2 {
        draw_tabs(frame, app, inner.x, linha_y);
        draw_chips(frame, app, inner);
    }
}

fn draw_tabs(frame: &mut Frame, app: &mut App, x0: u16, y: u16) {
    let theme = app.theme.clone();
    let mut spans: Vec<Span> = Vec::new();
    let mut hits: Vec<(Rect, Tab)> = Vec::new();
    let mut x = x0 + 1;
    for tab in Tab::ALL.iter() {
        let ativo = *tab == app.tab;
        let label = format!(" {} ", tab.title());
        let style = if ativo {
            Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        let width = label.chars().count() as u16;
        hits.push((
            Rect {
                x,
                y,
                width,
                height: 1,
            },
            *tab,
        ));
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
        x += width + 1;
    }
    app.tab_hits = hits;
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x: x0,
            y,
            width: x.saturating_sub(x0).max(1),
            height: 1,
        },
    );
}

fn draw_chips(frame: &mut Frame, app: &mut App, inner: Rect) {
    let theme = app.theme.clone();
    let chips = chip_labels(&app.filter);
    if chips.is_empty() {
        return;
    }

    let tabs_width = app
        .tab_hits
        .last()
        .map(|(rect, _)| rect.x + rect.width)
        .map(|x| x.saturating_sub(inner.x) + 1)
        .unwrap_or(2);
    let mut x = inner.x + tabs_width;
    let y = inner.y + 1;
    let limite = inner.x + inner.width;
    let mut spans: Vec<Span> = Vec::new();
    let mut rects: Vec<(Rect, ChipClear)> = Vec::new();

    for (label, clear) in chips {
        let texto = format!(" {label} × ");
        let width = texto.chars().count() as u16;
        if x + width > limite {
            break;
        }
        rects.push((
            Rect {
                x,
                y,
                width,
                height: 1,
            },
            clear,
        ));
        spans.push(Span::styled(
            texto,
            Style::default().fg(theme.accent).bg(theme.surface_alt),
        ));
        spans.push(Span::raw(" "));
        x += width + 1;
    }

    app.chips.retain(|(rect, _)| rect.y != y);
    app.chips.extend(rects);
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x: inner.x + tabs_width,
            y,
            width: inner.width.saturating_sub(tabs_width),
            height: 1,
        },
    );
}

/// Chips na ordem em que aparecem, com a ação de limpar correspondente.
pub(crate) fn chip_labels(filter: &dnfm_core::Filter) -> Vec<(String, ChipClear)> {
    let mut out: Vec<(String, ChipClear)> = Vec::new();
    if !filter.text.is_empty() {
        out.push((format!("\"{}\"", filter.text), ChipClear::Text));
    }
    for categoria in &filter.categories {
        out.push((categoria.clone(), ChipClear::Category(categoria.clone())));
    }
    for familia in &filter.families {
        out.push((familia.clone(), ChipClear::Family(familia.clone())));
    }
    if let Some(repo) = &filter.repo {
        out.push((format!("repo: {repo}"), ChipClear::Repo));
    }
    if let Some(reason) = filter.reason {
        out.push((format!("motivo: {}", reason.label()), ChipClear::Reason));
    }
    if let Some(arch) = &filter.arch {
        out.push((format!("arch: {arch}"), ChipClear::Arch));
    }
    if filter.min_size.is_some() || filter.max_size.is_some() {
        out.push((filter.size_preset.label().to_string(), ChipClear::Size));
    }
    if filter.installed_since.is_some() {
        out.push((filter.date_preset.label().to_string(), ChipClear::Date));
    }
    if filter.marked_only {
        out.push(("marcados".to_string(), ChipClear::Marked));
    }
    if let Some(protegido) = filter.protected {
        out.push((
            if protegido {
                "protegidos"
            } else {
                "não protegidos"
            }
            .to_string(),
            ChipClear::Protected,
        ));
    }
    if filter.only_updates {
        out.push(("com atualização".to_string(), ChipClear::Updates));
    }
    out
}
