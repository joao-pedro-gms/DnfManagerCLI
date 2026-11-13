//! Tradução de teclado e mouse em comandos. Sem I/O, testável isoladamente.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use dnfm_core::categories::GroupMode;

use crate::app::{App, Cmd, Modal, RowRef, Tab};

fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
}

/// Tecla -> comandos. Um evento pode gerar mais de um efeito.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Cmd> {
    if is_ctrl_c(&key) {
        app.quit = true;
        return vec![Cmd::Quit];
    }

    // Enquanto o modal fecha, o teclado fica bloqueado.
    if app.anim.closing {
        return Vec::new();
    }

    // Modais capturam o teclado.
    if let Some(modal) = &app.modal {
        match modal {
            Modal::Help | Modal::Error(_) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?')
                ) {
                    app.close_modal();
                }
                return Vec::new();
            }
            Modal::Preview(_) => {
                return match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        app.confirm_preview().into_iter().collect()
                    }
                    KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                        app.cancel_modal().into_iter().collect()
                    }
                    _ => Vec::new(),
                };
            }
            Modal::Progress => {
                if key.code == KeyCode::Esc {
                    app.status = "transação em andamento…".to_string();
                }
                return Vec::new();
            }
        }
    }

    // Campo de atribuição de categoria.
    if app.assign_input.is_some() {
        match key.code {
            KeyCode::Esc => app.assign_input = None,
            KeyCode::Enter => app.assign_input_commit(),
            KeyCode::Backspace => app.assign_input_pop(),
            KeyCode::Char(ch) => app.assign_input_push(ch),
            _ => {}
        }
        return Vec::new();
    }

    // Seletor de categoria/família.
    if app.picker.is_some() {
        match key.code {
            KeyCode::Esc => app.picker = None,
            KeyCode::Enter => app.picker_apply(),
            KeyCode::Up => app.picker_move(-1),
            KeyCode::Down => app.picker_move(1),
            KeyCode::PageUp => app.picker_move(-8),
            KeyCode::PageDown => app.picker_move(8),
            KeyCode::Char(' ') => app.picker_toggle(),
            KeyCode::Backspace => app.picker_backspace(),
            KeyCode::Char(ch) => app.picker_input(ch),
            _ => {}
        }
        return Vec::new();
    }

    // Painel de filtros.
    if app.filter_panel.is_some() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('f') => app.close_filter_panel(),
            KeyCode::Up | KeyCode::Char('k') => app.filter_panel_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.filter_panel_move(1),
            KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Right => {
                return app.activate_facet().into_iter().collect();
            }
            _ => {}
        }
        return Vec::new();
    }

    // Modo de busca.
    if let Some(buffer) = app.search.as_mut() {
        match key.code {
            KeyCode::Esc => {
                app.search = None;
                app.filter.text.clear();
                app.refresh();
            }
            KeyCode::Enter => {
                let text = buffer.clone();
                app.search = None;
                app.filter.text = text;
                app.refresh();
            }
            KeyCode::Backspace => {
                buffer.pop();
                app.filter.text = buffer.clone();
                app.refresh();
            }
            KeyCode::Char(ch) => {
                buffer.push(ch);
                app.filter.text = buffer.clone();
                app.refresh();
            }
            _ => {}
        }
        return Vec::new();
    }

    match key.code {
        KeyCode::Char('q') => {
            app.quit = true;
            vec![Cmd::Quit]
        }
        KeyCode::Esc => {
            app.filter.clear();
            app.collapsed.clear();
            app.after_filter_change();
            app.status = "filtros limpos".to_string();
            Vec::new()
        }
        KeyCode::Tab => {
            let next = app.tab.step(1);
            app.switch_tab(next);
            Vec::new()
        }
        KeyCode::BackTab => {
            let prev = app.tab.step(-1);
            app.switch_tab(prev);
            Vec::new()
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection(1);
            Vec::new()
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection(-1);
            Vec::new()
        }
        KeyCode::PageDown => {
            app.move_selection(app.viewport as isize);
            Vec::new()
        }
        KeyCode::PageUp => {
            app.move_selection(-(app.viewport as isize));
            Vec::new()
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.select(0);
            Vec::new()
        }
        KeyCode::F(2) => {
            app.cycle_theme();
            Vec::new()
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_sidebar();
            Vec::new()
        }
        KeyCode::Char('[') => {
            app.sidebar_scrub(-1);
            Vec::new()
        }
        KeyCode::Char(']') => {
            app.sidebar_scrub(1);
            Vec::new()
        }
        KeyCode::Char('0') => {
            app.sidebar_clear();
            Vec::new()
        }
        KeyCode::Char('o') => {
            cycle_group(app, false);
            Vec::new()
        }
        KeyCode::Char('O') => {
            cycle_group(app, true);
            Vec::new()
        }
        KeyCode::End | KeyCode::Char('G') => {
            let last = app.rows_len().saturating_sub(1);
            app.select(last);
            Vec::new()
        }
        KeyCode::Char(' ') => {
            if app.tab.marks_packages() {
                app.toggle_current();
            }
            Vec::new()
        }
        KeyCode::Char('a') => {
            if app.tab.marks_packages() {
                app.mark_all_visible();
            }
            Vec::new()
        }
        KeyCode::Char('u') => {
            app.clear_marks();
            Vec::new()
        }
        KeyCode::Char('/') => {
            app.search = Some(app.filter.text.clone());
            Vec::new()
        }
        KeyCode::Char('f') => {
            app.open_filter_panel();
            Vec::new()
        }
        KeyCode::Char('c') => {
            app.open_picker(crate::app::PickerTarget::Category);
            Vec::new()
        }
        KeyCode::Char('C') => {
            app.open_picker(crate::app::PickerTarget::Family);
            Vec::new()
        }
        KeyCode::Char('t') => {
            app.assign_input_start();
            Vec::new()
        }
        KeyCode::Char('T') => {
            app.unassign_current();
            Vec::new()
        }
        KeyCode::Char('s') => {
            app.sort = app.sort.next();
            app.refresh();
            app.status = format!("ordenado por {}", app.sort.label());
            Vec::new()
        }
        KeyCode::Char('S') => {
            app.descending = !app.descending;
            app.refresh();
            Vec::new()
        }
        KeyCode::Char('p') => app.request_preview().into_iter().collect(),
        KeyCode::Char('P') => {
            if app.tab == Tab::Updates {
                let specs: Vec<String> = if app.selected_upgrades.is_empty() {
                    app.current_upgrade()
                        .map(|u| vec![u.installed.name.clone()])
                        .unwrap_or_default()
                } else {
                    app.selected_upgrades.iter().cloned().collect()
                };
                if specs.is_empty() {
                    app.status = "nenhuma atualização selecionada".to_string();
                    return Vec::new();
                }
                app.loading = true;
                app.pending_specs = specs.clone();
                app.status = format!("resolvendo atualização de {} pacote(s)…", specs.len());
                vec![Cmd::PreviewUpgrade(specs)]
            } else {
                Vec::new()
            }
        }
        KeyCode::Enter => {
            if app.tab == Tab::Updates {
                app.toggle_upgrade_selection();
                Vec::new()
            } else if let Some(pkg) = app.current_package().map(|p| p.nevra.clone()) {
                vec![Cmd::LoadDetails(pkg)]
            } else {
                Vec::new()
            }
        }
        KeyCode::Char('!') => {
            if app.tab == Tab::Updates {
                app.select_all_upgrades(true);
            }
            Vec::new()
        }
        KeyCode::Char('A') => {
            if app.tab == Tab::Updates {
                app.select_all_upgrades(false);
            }
            Vec::new()
        }
        KeyCode::Char('r') => match app.tab {
            Tab::History => vec![Cmd::LoadHistory],
            Tab::Orphans => vec![Cmd::LoadOrphans],
            _ => vec![Cmd::LoadAll],
        },
        KeyCode::Char('?') | KeyCode::Char('h') => {
            app.open_modal(Modal::Help);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Mouse -> comandos. Hover realça, clique em chip remove o filtro, clique no
/// sidebar filtra pelo grupo, clique no cabeçalho ordena.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Vec<Cmd> {
    fn hit(rect: ratatui::layout::Rect, x: u16, y: u16) -> bool {
        rect.width > 0
            && rect.height > 0
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
    }

    let (x, y) = (mouse.column, mouse.row);
    let area = app.table_area;
    let inside_table = hit(area, x, y);

    match mouse.kind {
        MouseEventKind::Moved => {
            app.hover = if inside_table {
                let header_y = area.y + 1;
                if y > header_y {
                    let row = (y - header_y - 1) as usize + app.scroll;
                    (row < app.rows_len()).then_some(row)
                } else {
                    None
                }
            } else {
                None
            };
        }
        MouseEventKind::ScrollDown => {
            if hit(app.sidebar_area, x, y) {
                app.sidebar_move(1);
            } else {
                app.move_selection(3);
            }
        }
        MouseEventKind::ScrollUp => {
            if hit(app.sidebar_area, x, y) {
                app.sidebar_move(-1);
            } else {
                app.move_selection(-3);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Chips de filtro no cabeçalho.
            if let Some((_, chip)) = app.chips.iter().find(|(rect, _)| hit(*rect, x, y)).cloned() {
                app.clear_chip(&chip);
                return Vec::new();
            }

            // Sidebar: seleciona e filtra pelo grupo.
            if app.sidebar.visible && hit(app.sidebar_area, x, y) {
                let row = (y - app.sidebar_area.y) as usize + app.sidebar_scroll;
                if row < app.sidebar.buckets.len() {
                    app.sidebar.cursor = row;
                    app.sidebar_toggle();
                }
                return Vec::new();
            }

            // Abas (primeira linha do cabeçalho).
            if y <= 2 {
                let offset = (x as usize).saturating_sub(area.x as usize + 2);
                if let Some(tab) = Tab::ALL.get(offset / 18) {
                    app.switch_tab(*tab);
                }
                return Vec::new();
            }
            if !inside_table {
                return Vec::new();
            }
            let header_y = area.y + 1;
            if y == header_y {
                app.sort = match (x - area.x) / 18 {
                    1 => dnfm_core::SortKey::Name,
                    2 => dnfm_core::SortKey::Size,
                    3 => dnfm_core::SortKey::Repo,
                    4 => dnfm_core::SortKey::Category,
                    _ => app.sort.next(),
                };
                app.refresh();
                return Vec::new();
            }
            let row = (y - header_y - 1) as usize + app.scroll;
            if row >= app.rows_len() {
                return Vec::new();
            }
            if app.tab == Tab::Installed {
                if let Some(RowRef::Header { label, .. }) = app.rows.get(row).cloned() {
                    app.toggle_group(&label);
                    return Vec::new();
                }
            }
            app.select(row);
            if app.tab.marks_packages() && x < area.x + 4 {
                app.toggle_mark_current();
            }
        }
        _ => {}
    }
    Vec::new()
}

/// Cicla o agrupamento (`g` com shift alterna para trás).
pub fn cycle_group(app: &mut App, backwards: bool) {
    if backwards {
        for _ in 0..(GroupMode::ALL.len() - 1) {
            app.group = app.group.next();
        }
    } else {
        app.group = app.group.next();
    }
    app.collapsed.clear();
    app.rebuild_rows();
    app.ensure_package_selection();
    app.status = format!("agrupado por {}", app.group.label());
}
