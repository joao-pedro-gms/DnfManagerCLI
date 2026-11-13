//! Tradução de teclado e mouse em comandos. Sem I/O, testável isoladamente.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, Cmd, Modal, Tab};

fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
}

/// Tecla -> comandos. Um evento pode gerar mais de um efeito.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Cmd> {
    if is_ctrl_c(&key) {
        app.quit = true;
        return vec![Cmd::Quit];
    }

    // Modais capturam o teclado.
    if let Some(modal) = &app.modal {
        match modal {
            Modal::Help | Modal::Error(_) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?')
                ) {
                    app.modal = None;
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
                // Durante a transação só é possível sair da tela de progresso.
                if key.code == KeyCode::Esc {
                    app.status = "transação em andamento…".to_string();
                }
                return Vec::new();
            }
        }
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
            KeyCode::Char(c) => {
                buffer.push(c);
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
            app.refresh();
            Vec::new()
        }
        KeyCode::Tab => {
            app.tab = app.tab.step(1);
            app.scroll = 0;
            app.table.select(Some(0));
            Vec::new()
        }
        KeyCode::BackTab => {
            app.tab = app.tab.step(-1);
            app.scroll = 0;
            app.table.select(Some(0));
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
        KeyCode::End | KeyCode::Char('G') => {
            let last = app.rows_len().saturating_sub(1);
            app.select(last);
            Vec::new()
        }
        KeyCode::Char(' ') => {
            if app.tab.marks_packages() {
                app.toggle_mark_current();
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
            Tab::Installed => vec![Cmd::LoadAll],
            Tab::Updates => vec![Cmd::LoadAll],
            Tab::Orphans => vec![Cmd::LoadOrphans],
            Tab::History => vec![Cmd::LoadHistory],
            Tab::Repos => vec![Cmd::LoadAll],
        },
        KeyCode::Char('?') | KeyCode::Char('h') => {
            app.modal = Some(Modal::Help);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Mouse -> comandos. Clique na coluna da caixa marca; clique no cabeçalho ordena.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Vec<Cmd> {
    let area = app.table_area;
    let inside = mouse.column >= area.x
        && mouse.column < area.x + area.width
        && mouse.row >= area.y
        && mouse.row < area.y + area.height;

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            app.move_selection(3);
        }
        MouseEventKind::ScrollUp => {
            app.move_selection(-3);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Aba: primeira linha do cabeçalho.
            if mouse.row <= 2 {
                let offset = (mouse.column as usize).saturating_sub(area.x as usize + 2);
                let tab_width = 18;
                let idx = offset / tab_width;
                if let Some(tab) = Tab::ALL.get(idx) {
                    app.tab = *tab;
                    app.scroll = 0;
                    app.table.select(Some(0));
                }
                return Vec::new();
            }
            if !inside {
                return Vec::new();
            }
            // Cabeçalho da tabela: ordena pela coluna clicada.
            let header_y = area.y + 1;
            if mouse.row == header_y {
                app.sort = match (mouse.column - area.x) / 18 {
                    1 => dnfm_core::SortKey::Name,
                    2 => dnfm_core::SortKey::Size,
                    3 => dnfm_core::SortKey::Repo,
                    _ => app.sort.next(),
                };
                app.refresh();
                return Vec::new();
            }
            let row = (mouse.row - header_y - 1) as usize + app.scroll;
            if row < app.rows_len() {
                app.select(row);
                // Clique na coluna da caixa (ou nas duas primeiras células).
                if app.tab.marks_packages() && mouse.column < area.x + 4 {
                    app.toggle_mark_current();
                }
            }
        }
        _ => {}
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnfm_core::{Package, Reason};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn app() -> App {
        let mut app = App {
            installed: vec![Package {
                name: "vlc".into(),
                evr: "3.0.24-3.fc45".into(),
                arch: "x86_64".into(),
                repo_id: "@System".into(),
                from_repo_id: None,
                size: 1024,
                install_time: 0,
                summary: String::new(),
                reason: Reason::User,
                nevra: "vlc-3.0.24-3.fc45.x86_64".into(),
                is_installed: true,
            }],
            ..Default::default()
        };
        app.refresh();
        app
    }

    #[test]
    fn espaco_marca_pacote_sob_o_cursor() {
        let mut app = app();
        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert_eq!(app.marks.len(), 1);
    }

    #[test]
    fn barra_abre_busca_e_filtra_ao_digitar() {
        let mut app = app();
        handle_key(&mut app, key(KeyCode::Char('/')));
        assert!(app.search.is_some());
        handle_key(&mut app, key(KeyCode::Char('v')));
        assert_eq!(app.visible.len(), 1);
        handle_key(&mut app, key(KeyCode::Esc));
        assert!(app.search.is_none());
        assert!(app.filter.text.is_empty());
    }

    #[test]
    fn q_sai_mas_esc_so_limpa_filtro() {
        let mut app = app();
        assert!(handle_key(&mut app, key(KeyCode::Esc)).is_empty());
        assert!(!app.quit);
        let cmds = handle_key(&mut app, key(KeyCode::Char('q')));
        assert!(matches!(cmds.as_slice(), [Cmd::Quit]));
        assert!(app.quit);
    }

    #[test]
    fn p_gera_preview_com_a_marcacao() {
        let mut app = app();
        handle_key(&mut app, key(KeyCode::Char(' ')));
        let cmds = handle_key(&mut app, key(KeyCode::Char('p')));
        assert!(matches!(cmds.as_slice(), [Cmd::Preview(_)]));
    }

    #[test]
    fn tab_troca_de_aba() {
        let mut app = app();
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.tab, Tab::Updates);
    }

    #[test]
    fn ajuda_abre_e_fecha() {
        let mut app = app();
        handle_key(&mut app, key(KeyCode::Char('?')));
        assert!(matches!(app.modal, Some(Modal::Help)));
        handle_key(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none());
    }
}
