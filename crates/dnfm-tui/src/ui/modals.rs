//! Modais e sobreposições, com tema e animação de entrada.

use dnfm_core::format_size;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, FilterFacet, Picker};
use crate::theme::Theme;

/// Retângulo central com escala (0..1) para a animação de abertura.
pub(crate) fn modal_rect(area: Rect, geometry: (u16, u16), scale: f32) -> Rect {
    let scale = 0.88 + 0.12 * scale.clamp(0.0, 1.0);
    let (pct_w, height) = geometry;
    let largura = (area.width as f32 * pct_w as f32 / 100.0).min(area.width as f32) * scale;
    let altura = (height as f32).min(area.height as f32) * scale;
    let largura = (largura as u16).clamp(20, area.width.max(20));
    let altura = (altura as u16).clamp(5, area.height.max(5));
    Rect {
        x: area.x + area.width.saturating_sub(largura) / 2,
        y: area.y + area.height.saturating_sub(altura) / 2,
        width: largura,
        height: altura,
    }
}

fn shadow(frame: &mut Frame, rect: Rect, theme: &Theme) {
    let sombra = Rect {
        x: rect.x.saturating_add(2),
        y: rect.y.saturating_add(1),
        width: rect.width,
        height: rect.height,
    };
    frame.render_widget(Clear, sombra);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.surface)),
        sombra,
    );
}

/// Bloco padrão de modal: borda arredondada, fundo próprio e título.
fn modal_block<'a>(theme: &Theme, title: &'a str, accent: Color, pulse: f32) -> Block<'a> {
    Block::bordered()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(if pulse > 0.0 {
                    theme.pulsed(pulse)
                } else {
                    accent
                })
                .add_modifier(Modifier::BOLD),
        ))
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme.surface))
}

pub(crate) fn draw_help(frame: &mut Frame, app: &App, rect: Rect) {
    let theme = &app.theme;
    let rows = [
        ("espaço", "marca o pacote / recolhe o grupo"),
        ("a / u", "marca os visíveis / limpa marcações"),
        ("o / O", "agrupa por categoria, família, repo, motivo"),
        ("[ / ]", "percorre os grupos do sidebar aplicando filtro"),
        ("0", "limpa o filtro do grupo atual"),
        ("f", "painel de filtros (12 facetas)"),
        ("c / C", "filtra por categoria / família"),
        ("t / T", "dá ou remove uma categoria própria"),
        ("p / P", "preview de remoção / atualização"),
        ("enter", "detalhes (ou seleciona atualização)"),
        ("! / A", "atualizações de segurança / todas"),
        ("/", "busca fuzzy por nome e resumo"),
        ("s / S", "cicla ordenação / inverte"),
        ("F2", "troca a paleta de cores"),
        ("ctrl+b", "mostra/oculta o sidebar"),
        ("tab", "próxima aba"),
        ("q", "sai"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("  {key:<9}"),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(theme.text)),
            ])
        })
        .collect();

    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(modal_block(
                theme,
                "ajuda — esc fecha",
                theme.accent,
                app.anim.pulse_level(),
            )),
        rect,
    );
}

pub(crate) fn draw_error(frame: &mut Frame, app: &App, rect: Rect, message: &str) {
    let theme = &app.theme;
    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(Span::styled(
            message.to_string(),
            Style::default().fg(theme.text),
        ))
        .wrap(Wrap { trim: true })
        .block(modal_block(
            theme,
            "erro — esc fecha",
            theme.danger,
            app.anim.pulse_level(),
        )),
        rect,
    );
}

pub(crate) fn draw_preview(
    frame: &mut Frame,
    app: &App,
    rect: Rect,
    preview: &dnfm_core::Preview,
    specs: &[String],
) {
    let theme = &app.theme;
    let mut lines: Vec<Line> = Vec::new();
    let requested = preview.requested();
    let collateral = preview.collateral();
    let additions: Vec<_> = preview.additions().collect();

    if preview.is_failed() && !specs.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("pedido: ", Style::default().fg(theme.dim)),
            Span::styled(
                specs.join(" "),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "VOCÊ MARCOU",
            Style::default().fg(theme.dim).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", requested.len()),
            Style::default().fg(theme.mark),
        ),
    ]));
    for item in requested.iter().take(5) {
        lines.push(Line::from(vec![
            Span::styled("  − ", Style::default().fg(theme.mark)),
            Span::styled(item.package.name.clone(), Style::default().fg(theme.text)),
        ]));
    }
    if requested.len() > 5 {
        lines.push(Line::from(Span::styled(
            format!("    … e mais {}", requested.len() - 5),
            Style::default().fg(theme.dim),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "SAEM JUNTO",
            Style::default().fg(theme.dim).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", collateral.len()),
            Style::default().fg(theme.danger),
        ),
    ]));
    for item in collateral.iter().take(5) {
        lines.push(Line::from(vec![
            Span::styled("  − ", Style::default().fg(theme.danger)),
            Span::styled(item.package.name.clone(), Style::default().fg(theme.text)),
            Span::styled(
                format!("  ({})", item.reason.label()),
                Style::default().fg(theme.dim),
            ),
        ]));
    }
    if collateral.len() > 5 {
        lines.push(Line::from(Span::styled(
            format!("    … e mais {}", collateral.len() - 5),
            Style::default().fg(theme.dim),
        )));
    }

    if !additions.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("ENTRAM  {}", additions.len()),
            Style::default().fg(theme.dim).add_modifier(Modifier::BOLD),
        )));
        for item in additions.iter().take(3) {
            lines.push(Line::from(vec![
                Span::styled("  + ", Style::default().fg(theme.ok)),
                Span::styled(item.package.name.clone(), Style::default().fg(theme.text)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("espaço liberado  ", Style::default().fg(theme.dim)),
        Span::styled(
            format_size(preview.freed_bytes()),
            Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
        ),
    ]));

    if !preview.problems.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "PROBLEMAS DE RESOLUÇÃO",
            Style::default()
                .fg(theme.danger)
                .add_modifier(Modifier::BOLD),
        )));
        for problem in preview.problems.iter().take(5) {
            lines.push(Line::from(Span::styled(
                format!("  {problem}"),
                Style::default().fg(theme.danger),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(if preview.is_failed() {
        Line::from(Span::styled(
            "resolução falhou — esc fecha",
            Style::default().fg(theme.danger),
        ))
    } else {
        Line::from(vec![
            Span::styled(
                " y ",
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.ok)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" aplica    ", Style::default().fg(theme.dim)),
            Span::styled(
                " n ",
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.warn)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancela", Style::default().fg(theme.dim)),
        ])
    });

    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(modal_block(
                theme,
                "preview da transação",
                theme.warn,
                app.anim.pulse_level(),
            )),
        rect,
    );
}

pub(crate) fn draw_progress(frame: &mut Frame, app: &App, rect: Rect) {
    let theme = &app.theme;
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
    let accent = if app.progress.success == Some(false) {
        theme.danger
    } else {
        theme.ok
    };

    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    let block = modal_block(theme, label, accent, app.anim.pulse_level());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let [medidor, log_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(2)]).areas(inner);

    let mut texto = app.progress.current.clone();
    if app.progress.success.is_none() {
        texto = format!("{} {texto}", app.anim.spinner());
    }
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(accent).bg(theme.surface_alt))
            .ratio(percent)
            .label(Span::styled(
                format!("{:.0}%", percent * 100.0),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            )),
        medidor,
    );

    let mut linhas = vec![Line::from(Span::styled(
        texto,
        Style::default().fg(theme.text),
    ))];
    for entry in app.progress.log.iter().rev().take(3).rev() {
        linhas.push(Line::from(Span::styled(
            entry.clone(),
            Style::default().fg(theme.dim),
        )));
    }
    frame.render_widget(Paragraph::new(linhas), log_area);
}

/// Painel de filtros: uma linha por faceta, com o valor à direita.
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

fn centered(area: Rect, percent_x: u16, height: u16) -> Rect {
    let height = height.min(area.height);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .areas(area);
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escala_do_modal_cresce_ate_o_tamanho_final() {
        let area = Rect::new(0, 0, 120, 40);
        let pequeno = modal_rect(area, (70, 30), 0.0);
        let grande = modal_rect(area, (70, 30), 1.0);
        assert!(pequeno.width < grande.width);
        assert!(pequeno.height < grande.height);
        assert_eq!(grande.width, 84);
        assert_eq!(grande.height, 30);
    }

    #[test]
    fn modal_cabe_em_terminal_minusculo() {
        let area = Rect::new(0, 0, 20, 5);
        let rect = modal_rect(area, (80, 40), 1.0);
        assert!(rect.width <= area.width);
        assert!(rect.height <= area.height);
        assert!(rect.x + rect.width <= area.width);
    }

    #[test]
    fn escala_zero_nunca_zera_o_modal() {
        let area = Rect::new(0, 0, 100, 30);
        let rect = modal_rect(area, (60, 20), 0.0);
        assert!(rect.width >= 20);
        assert!(rect.height >= 5);
    }
}
