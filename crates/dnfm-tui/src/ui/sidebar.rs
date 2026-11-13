//! Sidebar de grupos: barras proporcionais, seleção e navegação por teclado.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;

use crate::app::App;

const BAR_WIDTH: usize = 7;

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.clone();
    let selecionados = app.sidebar.selected_count();
    let titulo = if selecionados > 0 {
        format!(" {} · {} ativo(s) ", app.sidebar.title, selecionados)
    } else {
        format!(" {} ", app.sidebar.title)
    };
    let block = Block::bordered()
        .title(Span::styled(
            titulo,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .border_type(BorderType::Rounded)
        .border_style(theme.border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.sidebar_area = inner;
    if inner.height < 2 {
        return;
    }

    let lista_altura = inner.height.saturating_sub(2) as usize;
    let cursor = app.sidebar.cursor;
    let inicio = cursor.saturating_sub(lista_altura.saturating_sub(1));
    app.sidebar_scroll = inicio;

    let max = app.sidebar.max_count().max(1);
    let largura_rotulo = inner.width.saturating_sub(BAR_WIDTH as u16 + 9) as usize;
    let barras = app.anim.bars;
    let pulse = app.anim.pulse_level();

    for (offset, bucket) in app
        .sidebar
        .buckets
        .iter()
        .skip(inicio)
        .take(lista_altura)
        .enumerate()
    {
        let y = inner.y + offset as u16;
        let em_foco = inicio + offset == cursor;
        let rotulo = truncate(&bucket.label, largura_rotulo);
        let preenchido =
            ((bucket.count as f32 / max as f32) * BAR_WIDTH as f32 * barras).round() as usize;
        let preenchido = preenchido.min(BAR_WIDTH);
        // Trilha discreta ("·") e preenchimento em bloco: lê como gráfico,
        // não como uma barra sólida.
        let barra = format!(
            "{}{}",
            "█".repeat(preenchido),
            "·".repeat(BAR_WIDTH - preenchido)
        );

        let cor_rotulo = if bucket.selected {
            theme.accent
        } else if em_foco {
            theme.text
        } else {
            theme.dim
        };
        let cor_barra = if bucket.selected {
            theme.accent
        } else if em_foco {
            theme.text
        } else {
            theme.dim
        };
        let cor_trilha = theme.surface_alt;
        let mut linha = Line::from(vec![
            Span::styled(
                if bucket.selected { "▸" } else { " " },
                Style::default().fg(theme.accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{rotulo:<largura$}", largura = largura_rotulo),
                Style::default()
                    .fg(cor_rotulo)
                    .add_modifier(if bucket.selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::raw(" "),
            Span::styled(barra, Style::default().fg(cor_barra).bg(cor_trilha)),
            Span::styled(
                format!(" {:>5}", bucket.count),
                Style::default().fg(if bucket.selected {
                    theme.text
                } else {
                    theme.dim
                }),
            ),
        ]);
        if em_foco || offset % 2 == 1 {
            linha = linha.style(Style::default().bg(theme.surface));
        }

        // Pulso suave no grupo selecionado logo após uma mudança.
        if bucket.selected && pulse > 0.0 {
            linha = linha.style(Style::default().bg(theme.surface_alt));
        }

        frame.render_widget(
            Paragraph::new(linha),
            Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            },
        );
    }

    // Resumo no espaço que sobra, se houver.
    let usado = lista_altura.min(app.sidebar.buckets.len().saturating_sub(inicio));
    let sobra = lista_altura.saturating_sub(usado);
    if sobra >= 5 {
        let base = inner.y + usado as u16 + 1;
        let maior = app
            .sidebar
            .buckets
            .first()
            .map(|b| b.label.clone())
            .unwrap_or_else(|| "—".into());
        let linhas = [
            ("total", app.sidebar.total.to_string()),
            ("marcados", app.marks.len().to_string()),
            ("maior grupo", maior),
            ("filtros", app.filter.active_count().to_string()),
        ];
        for (i, (rotulo, valor)) in linhas.iter().enumerate() {
            if i >= sobra.saturating_sub(1) {
                break;
            }
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {rotulo:<12}"), Style::default().fg(theme.dim)),
                    Span::styled(valor.clone(), Style::default().fg(theme.text)),
                ])),
                Rect {
                    x: inner.x,
                    y: base + i as u16,
                    width: inner.width,
                    height: 1,
                },
            );
        }
    }

    // Rodapé do painel.
    let rodape_y = inner.y + inner.height - 1;
    let total: usize = app.sidebar.buckets.len();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{total} grupos"), Style::default().fg(theme.dim)),
            Span::styled("  ·  [ ] navega", Style::default().fg(theme.dim)),
        ])),
        Rect {
            x: inner.x,
            y: rodape_y,
            width: inner.width,
            height: 1,
        },
    );
}

fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        text.to_string()
    } else if width <= 1 {
        "…".to_string()
    } else {
        let mut out: String = chars[..width - 1].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncacao_respeita_a_largura() {
        assert_eq!(truncate("TeX Live", 20), "TeX Live");
        assert_eq!(truncate("TeX Live", 4), "TeX…");
        assert_eq!(truncate("TeX Live", 1), "…");
        assert_eq!(truncate("TeX Live", 0), "");
    }
}
