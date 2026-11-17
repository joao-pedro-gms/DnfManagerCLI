//! Aba de estatísticas: visão agregada do sistema instalado.

use dnfm_core::{format_size, Bucket, Stats};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::theme::Theme;

const BAR_WIDTH: usize = 12;

#[derive(Debug, Clone, Copy)]
struct BucketOpts {
    com_tamanho: bool,
    limite: usize,
}

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    app.ensure_stats();
    let stats = app.stats.clone();
    let theme = app.theme.clone();

    let [esquerda, direita] =
        Layout::horizontal([Constraint::Percentage(52), Constraint::Min(30)]).areas(area);

    let [resumo, categorias] =
        Layout::vertical([Constraint::Length(11), Constraint::Min(8)]).areas(esquerda);
    let [tipos, maiores, atualizacoes] = Layout::vertical([
        Constraint::Length((stats.signals.len() as u16 + 3).clamp(5, 12)),
        Constraint::Length(13),
        Constraint::Min(6),
    ])
    .areas(direita);

    draw_resumo(frame, &theme, resumo, &stats);
    draw_buckets(
        frame,
        &theme,
        categorias,
        "CATEGORIAS",
        &stats.by_category,
        stats.total,
        BucketOpts {
            com_tamanho: true,
            limite: 14,
        },
    );
    draw_buckets(
        frame,
        &theme,
        tipos,
        "O QUE É CADA PACOTE",
        &stats.signals,
        stats.total,
        BucketOpts {
            com_tamanho: false,
            limite: 10,
        },
    );
    draw_biggest(frame, &theme, maiores, &stats);
    draw_updates(frame, &theme, atualizacoes, &stats);
}

fn draw_resumo(frame: &mut Frame, theme: &Theme, area: Rect, stats: &Stats) {
    let mut linhas = vec![
        campo(theme, "pacotes instalados", &reais(stats.total)),
        campo(theme, "nomes únicos", &reais(stats.unique_names)),
        campo(theme, "tamanho em disco", &format_size(stats.total_size)),
        campo(
            theme,
            "média por pacote",
            &format_size(stats.total_size / stats.total.max(1) as u64),
        ),
        campo(theme, "protegidos", &reais(stats.protected)),
        campo(theme, "marcados para sair", &reais(stats.marked)),
        campo(theme, "tamanho marcado", &format_size(stats.marked_size)),
        campo(
            theme,
            "com AppStream",
            &format!(
                "{} ({:.1}%)",
                reais(stats.with_appstream),
                pct(stats.with_appstream, stats.total)
            ),
        ),
        campo(
            theme,
            "com arquivos",
            &format!(
                "{} ({:.1}%)",
                reais(stats.with_files),
                pct(stats.with_files, stats.total)
            ),
        ),
        motivo_linha(theme, stats),
    ];
    linhas.truncate(area.height.saturating_sub(2) as usize);
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, "RESUMO", theme.accent)),
        area,
    );
}

fn motivo_linha<'a>(theme: &Theme, stats: &Stats) -> Line<'a> {
    let texto = stats
        .by_reason
        .iter()
        .map(|b| format!("{} {}", b.count, b.label))
        .collect::<Vec<_>>()
        .join(" · ");
    Line::from(vec![
        Span::styled("  motivo          ", Style::default().fg(theme.dim)),
        Span::styled(texto, Style::default().fg(theme.text)),
    ])
}

fn draw_buckets(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    titulo: &str,
    buckets: &[Bucket],
    total: usize,
    opts: BucketOpts,
) {
    let altura = area.height.saturating_sub(2) as usize;
    let mut linhas: Vec<Line> = Vec::new();
    for bucket in buckets.iter().take(opts.limite).take(altura) {
        let share = bucket.share(total);
        let mut spans = vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                format!("{:<16}", truncar(&bucket.label, 16)),
                Style::default().fg(theme.text),
            ),
            Span::styled(barra(share), Style::default().fg(theme.accent)),
            Span::styled(
                format!(" {:>5}", reais(bucket.count)),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {:>5.1}%", share * 100.0),
                Style::default().fg(theme.dim),
            ),
        ];
        if opts.com_tamanho {
            spans.push(Span::styled(
                format!(" {:>9}", format_size(bucket.size)),
                Style::default().fg(theme.dim),
            ));
        }
        linhas.push(Line::from(spans));
    }
    if linhas.is_empty() {
        linhas.push(Line::from(Span::styled(
            "  sem dados ainda",
            Style::default().fg(theme.dim),
        )));
    }
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, titulo, theme.accent_alt)),
        area,
    );
}

fn draw_biggest(frame: &mut Frame, theme: &Theme, area: Rect, stats: &Stats) {
    let altura = area.height.saturating_sub(2) as usize;
    let mut linhas: Vec<Line> = Vec::new();
    for (idx, bucket) in stats.biggest.iter().take(altura).enumerate() {
        linhas.push(Line::from(vec![
            Span::styled(
                format!("  {:>2}. {:<22}", idx + 1, truncar(&bucket.label, 22)),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:>9}", format_size(bucket.size)),
                Style::default().fg(theme.warn),
            ),
            Span::styled(
                if bucket.count > 1 {
                    format!("  {:>7}", versoes_label(bucket.count))
                } else {
                    String::new()
                },
                Style::default().fg(theme.dim),
            ),
        ]));
    }
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, "MAIORES PACOTES", theme.warn)),
        area,
    );
}

fn draw_updates(frame: &mut Frame, theme: &Theme, area: Rect, stats: &Stats) {
    let mut linhas = vec![
        campo(theme, "disponíveis", &reais(stats.upgrades)),
        campo(theme, "download", &format_size(stats.upgrade_bytes)),
    ];
    for bucket in &stats.by_severity {
        linhas.push(campo(theme, &bucket.label, &reais(bucket.count)));
    }
    linhas.truncate(area.height.saturating_sub(2) as usize);
    frame.render_widget(
        Paragraph::new(linhas).block(bloco(theme, "ATUALIZAÇÕES", theme.ok)),
        area,
    );
}

fn versoes_label(count: usize) -> String {
    format!("{count} {}", if count == 1 { "versão" } else { "versões" })
}

fn campo<'a>(theme: &Theme, rotulo: &str, valor: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {rotulo:<22}"), Style::default().fg(theme.dim)),
        Span::styled(
            valor.to_string(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn bloco<'a>(theme: &Theme, titulo: &'a str, cor: ratatui::style::Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(
            format!(" {titulo} "),
            Style::default().fg(cor).add_modifier(Modifier::BOLD),
        ))
}

fn barra(share: f32) -> String {
    let preenchidos = ((share.clamp(0.0, 1.0)) * BAR_WIDTH as f32).round() as usize;
    let cheio = "▰".repeat(preenchidos);
    let vazio = "▱".repeat(BAR_WIDTH - preenchidos);
    format!(" {cheio}{vazio}")
}

fn reais(n: usize) -> String {
    let texto = n.to_string();
    let mut saida = String::with_capacity(texto.len() + texto.len() / 3);
    for (idx, ch) in texto.chars().enumerate() {
        if idx > 0 && (texto.len() - idx) % 3 == 0 {
            saida.push('.');
        }
        saida.push(ch);
    }
    saida
}

fn pct(parte: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        parte as f32 * 100.0 / total as f32
    }
}

fn truncar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        return texto.to_string();
    }
    let mut saida: String = texto.chars().take(max.saturating_sub(1)).collect();
    saida.push('…');
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotula_versoes_no_pacote_colapsado() {
        assert_eq!(versoes_label(1), "1 versão");
        assert_eq!(versoes_label(2), "2 versões");
    }

    #[test]
    fn barra_respeita_a_largura() {
        for share in [0.0, 0.25, 0.5, 0.99, 1.0] {
            let texto = barra(share);
            assert_eq!(texto.trim().chars().count(), BAR_WIDTH);
        }
    }

    #[test]
    fn numeros_usam_separador_de_milhar() {
        assert_eq!(reais(0), "0");
        assert_eq!(reais(999), "999");
        assert_eq!(reais(8009), "8.009");
        assert_eq!(reais(1234567), "1.234.567");
    }

    #[test]
    fn truncar_nao_estoura_o_limite() {
        assert_eq!(truncar("abc", 5), "abc");
        assert_eq!(truncar("texlive-fonts-extra", 8), "texlive…");
        assert_eq!(truncar("texlive-fonts-extra", 8).chars().count(), 8);
    }

    #[test]
    fn porcentagem_nao_divide_por_zero() {
        assert_eq!(pct(1, 0), 0.0);
        assert!((pct(1, 4) - 25.0).abs() < 0.01);
    }
}
