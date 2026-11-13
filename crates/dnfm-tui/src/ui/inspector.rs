//! Inspetor de pacote: metadados, categorias automáticas, arquivos e estado.

use dnfm_core::format_size;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Inspection};
use crate::theme::Theme;

use super::modals::{modal_block, shadow};

/// Inspetor de pacote: metadados, categorias automáticas, arquivos e estado.
pub(crate) fn draw(frame: &mut Frame, app: &App, rect: Rect) {
    let theme = &app.theme;
    let Some(insp) = app.inspection.as_ref() else {
        return;
    };
    let pkg = &insp.package;

    shadow(frame, rect, theme);
    frame.render_widget(Clear, rect);
    let bloco = modal_block(theme, "INSPETOR", theme.accent, app.anim.pulse_level());
    let interna = bloco.inner(rect);
    frame.render_widget(bloco, rect);

    let [topo, corpo, rodape] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .areas(interna);

    // Cabeçalho: nevra, repo, tamanho, motivo.
    let titulo = Line::from(vec![
        Span::styled(
            format!(" {} ", pkg.nevra),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("· {} ", pkg.repo_id),
            Style::default().fg(theme.dim),
        ),
        Span::styled(
            format!("· {} ", format_size(pkg.size)),
            Style::default().fg(theme.accent_alt),
        ),
        Span::styled(
            format!("· {}", pkg.reason.label()),
            Style::default().fg(theme.dim),
        ),
    ]);
    let resumo = Line::from(Span::styled(
        format!(" {}", pkg.summary),
        Style::default().fg(theme.text),
    ));
    frame.render_widget(
        Paragraph::new(vec![titulo, resumo]).wrap(Wrap { trim: true }),
        topo,
    );

    let [esquerda, direita] =
        Layout::horizontal([Constraint::Percentage(56), Constraint::Min(24)]).areas(corpo);
    let [metadados, categorias] =
        Layout::vertical([Constraint::Length(9), Constraint::Min(6)]).areas(esquerda);
    let [arquivos, estado] =
        Layout::vertical([Constraint::Length(12), Constraint::Min(6)]).areas(direita);

    draw_metadata(frame, theme, metadados, insp);
    draw_tags(frame, theme, categorias, app, insp);
    draw_files(frame, theme, arquivos, insp);
    draw_state(frame, theme, estado, app, insp);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " esc fecha · m marca · t categoria própria",
            Style::default().fg(theme.dim),
        ))),
        rodape,
    );
}

fn draw_metadata(frame: &mut Frame, theme: &Theme, area: Rect, insp: &Inspection) {
    let detalhes = insp.details.as_ref();
    let mut linhas = Vec::new();
    let largura_valor = (area.width as usize).saturating_sub(16).max(10);
    let linha = |rotulo: &str, valor: &str| {
        Line::from(vec![
            Span::styled(format!(" {rotulo:<12}"), Style::default().fg(theme.dim)),
            Span::styled(
                recortar(valor, largura_valor),
                Style::default().fg(theme.text),
            ),
        ])
    };
    match detalhes {
        Some(d) => {
            linhas.push(linha("licença", &d.license));
            linhas.push(linha("fornecedor", &d.vendor));
            linhas.push(linha("url", &d.url));
            if !d.group.is_empty() && d.group != "Unspecified" {
                linhas.push(linha("grupo rpm", &d.group));
            }
            let descricao: String = d
                .description
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let descricao = recortar(&descricao, 260);
            linhas.push(Line::from(""));
            linhas.push(Line::from(Span::styled(
                format!(" {}", recortar(&descricao, 240)),
                Style::default().fg(theme.dim),
            )));
        }
        None => linhas.push(Line::from(Span::styled(
            " carregando metadados…",
            Style::default().fg(theme.dim),
        ))),
    }
    frame.render_widget(
        Paragraph::new(linhas)
            .wrap(Wrap { trim: true })
            .block(bloco(theme, "METADADOS", theme.accent_alt)),
        area,
    );
}

fn draw_tags(frame: &mut Frame, theme: &Theme, area: Rect, app: &App, insp: &Inspection) {
    let mut linhas = Vec::new();
    for categoria in &insp.categories {
        linhas.push(Line::from(vec![
            Span::styled(" categoria  ", Style::default().fg(theme.dim)),
            Span::styled(
                categoria.label.clone(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", categoria.source.label()),
                Style::default().fg(theme.dim),
            ),
        ]));
    }
    if let Some(componente) = app.catalog.appstream_for(&insp.package.name) {
        linhas.push(Line::from(vec![
            Span::styled(" appstream   ", Style::default().fg(theme.dim)),
            Span::styled(componente.id.clone(), Style::default().fg(theme.dim)),
        ]));
    }
    if let Some(familia) = app.catalog.family_for(&insp.package.name) {
        linhas.push(Line::from(vec![
            Span::styled(" família url ", Style::default().fg(theme.dim)),
            Span::styled(familia.to_string(), Style::default().fg(theme.text)),
        ]));
    }
    if insp.tags.is_empty() {
        linhas.push(Line::from(Span::styled(
            " sem tags automáticas",
            Style::default().fg(theme.dim),
        )));
    } else {
        for origem in [
            dnfm_core::TagSource::AppStream,
            dnfm_core::TagSource::DesktopFile,
            dnfm_core::TagSource::FileSignal,
        ] {
            let rotulos: Vec<String> = insp
                .tags
                .iter()
                .filter(|t| t.source == origem)
                .map(|t| t.label.clone())
                .collect();
            if rotulos.is_empty() {
                continue;
            }
            linhas.push(Line::from(vec![
                Span::styled(
                    format!(" {:<11}", origem.label()),
                    Style::default().fg(theme.dim),
                ),
                Span::styled(rotulos.join(" · "), Style::default().fg(theme.text)),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(linhas)
            .wrap(Wrap { trim: true })
            .block(bloco(theme, "CATEGORIAS E TAGS", theme.accent)),
        area,
    );
}

fn draw_files(frame: &mut Frame, theme: &Theme, area: Rect, insp: &Inspection) {
    let mut linhas = Vec::new();
    match insp.files.as_ref() {
        Some(files) => {
            linhas.push(Line::from(vec![
                Span::styled(" arquivos    ", Style::default().fg(theme.dim)),
                Span::styled(
                    files.count.to_string(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ]));
            for (dir, count) in files.dirs.iter().take(6) {
                linhas.push(Line::from(vec![
                    Span::styled(
                        format!(" {:<26}", recortar(dir, 26)),
                        Style::default().fg(theme.text),
                    ),
                    Span::styled(format!("{count:>6}"), Style::default().fg(theme.dim)),
                ]));
            }
        }
        None => {
            linhas.push(Line::from(Span::styled(
                " lista de arquivos ainda não carregada",
                Style::default().fg(theme.dim),
            )));
            linhas.push(Line::from(Span::styled(
                " (r na aba Instalados recalcula)",
                Style::default().fg(theme.dim),
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, "ARQUIVOS", theme.warn)),
        area,
    );
}

fn draw_state(frame: &mut Frame, theme: &Theme, area: Rect, app: &App, insp: &Inspection) {
    let pkg = &insp.package;
    let mut linhas = Vec::new();
    let mut linha = |rotulo: &str, valor: String, cor: Color| {
        linhas.push(Line::from(vec![
            Span::styled(format!(" {rotulo:<12}"), Style::default().fg(theme.dim)),
            Span::styled(valor, Style::default().fg(cor)),
        ]));
    };

    if app.marks.contains(&pkg.name, &pkg.arch) {
        linha("marcado", "sim, sai na transação".to_string(), theme.danger);
    } else {
        linha("marcado", "não".to_string(), theme.dim);
    }
    if let Some(motivo) = dnfm_core::safety::is_protected(&pkg.name) {
        linha("protegido", motivo.to_string(), theme.warn);
    }
    if let Some(categoria) = app.custom_categories.get(&pkg.name, &pkg.arch) {
        linha("categoria", categoria.to_string(), theme.accent);
    }
    match app.upgrades.iter().find(|u| u.name() == pkg.name) {
        Some(up) => {
            linha(
                "atualização",
                format!("{} → {}", pkg.evr, up.candidate.evr),
                theme.ok,
            );
            linha("severidade", up.severity.label().to_string(), theme.ok);
        }
        None => linha("atualização", "em dia".to_string(), theme.dim),
    }
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, "ESTADO", theme.ok)),
        area,
    );
}

fn bloco<'a>(theme: &Theme, titulo: &'a str, cor: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(
            format!(" {titulo} "),
            Style::default().fg(cor).add_modifier(Modifier::BOLD),
        ))
}

fn recortar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        return texto.to_string();
    }
    let mut saida: String = texto.chars().take(max.saturating_sub(1)).collect();
    saida.push('…');
    saida
}
