use dnfm_core::format_date;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, RowRef};

pub(super) fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
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
        let detalhes = app.details.as_ref().filter(|d| d.nevra == pkg.nevra);
        if detalhes.is_none() {
            lines.push(Line::from(Span::styled(
                "enter: descrição, licença e URL",
                Style::default().fg(theme.dim),
            )));
        }
        if let Some(details) = detalhes {
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
