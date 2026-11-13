//! TUI do dnfm: laço de eventos, terminal e execução dos comandos.

pub mod anim;
pub mod app;
pub mod keys;
pub mod theme;
pub mod ui;

use std::io;
use std::sync::Arc;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use dnfm_core::{CustomCategories, DnfBackend, MarkSet, ProgressEvent};
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::app::{App, AppEvent, Cmd};

/// Restaura o terminal mesmo em panic ou saída antecipada.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn init_terminal() -> anyhow::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    Ok(terminal)
}

/// Roda a interface até o usuário sair.
pub async fn run(backend: Arc<dyn DnfBackend>) -> anyhow::Result<App> {
    let (ui, theme) = crate::theme::UiSettings::load();
    let mut app = App::new(MarkSet::load_default(), CustomCategories::load_default());
    app.theme = theme;
    app.ui = ui;
    let mut terminal = init_terminal()?;
    let _guard = TerminalGuard;

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut events = EventStream::new();

    spawn_cmd(&tx, &backend, Cmd::LoadAll);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        tokio::select! {
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        for cmd in keys::handle_key(&mut app, key) {
                            spawn_cmd(&tx, &backend, cmd);
                        }
                    }
                    Some(Ok(Event::Mouse(mouse))) => {
                        for cmd in keys::handle_mouse(&mut app, mouse) {
                            spawn_cmd(&tx, &backend, cmd);
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        tracing::warn!(%err, "erro lendo eventos do terminal");
                        break;
                    }
                    None => break,
                }
            }
            Some(event) = rx.recv() => {
                if let Some(cmd) = app.apply_event(event) {
                    spawn_cmd(&tx, &backend, cmd);
                }
                for cmd in app.extra_cmds.drain(..) {
                    spawn_cmd(&tx, &backend, cmd);
                }
                lazy_load(&mut app, &tx, &backend);
            }
            _ = tokio::time::sleep(app.anim.frame_time()) => {
                app.on_tick();
                persist(&mut app);
            }
        }

        lazy_load(&mut app, &tx, &backend);

        if app.quit {
            break;
        }
    }

    // Última gravação antes de sair.
    let _ = app.marks.save();
    let _ = app.custom_categories.save();
    let _ = app.ui.save(&app.theme);
    Ok(app)
}

/// Grava o que mudou (marcações, categorias, tema).
fn persist(app: &mut App) {
    if app.marks.is_dirty() {
        if let Err(err) = app.marks.save() {
            tracing::warn!(%err, "não consegui gravar as marcações");
            app.status = format!("erro ao gravar marcações: {err}");
        }
    }
    if app.custom_categories.is_dirty() {
        if let Err(err) = app.custom_categories.save() {
            tracing::warn!(%err, "não consegui gravar as categorias");
            app.status = format!("erro ao gravar categorias: {err}");
        }
    }
    if app.ui.dirty {
        let _ = app.ui.save(&app.theme);
    }
}

/// Carrega dados da aba apenas na primeira visita.
fn lazy_load(app: &mut App, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn DnfBackend>) {
    if let Some(cmd) = app.lazy_request() {
        spawn_cmd(tx, backend, cmd);
    }
}

fn spawn_cmd(tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn DnfBackend>, cmd: Cmd) {
    let tx = tx.clone();
    let backend = backend.clone();
    match cmd {
        Cmd::Quit => {}
        Cmd::LoadAll => {
            tokio::spawn(async move {
                match backend.installed().await {
                    Ok(list) => {
                        let _ = tx.send(AppEvent::Installed(list));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                        return;
                    }
                }
                if let Ok(upgrades) = backend.upgrades().await {
                    let _ = tx.send(AppEvent::Upgrades(upgrades));
                }
                if let Ok(repos) = backend.repos().await {
                    let _ = tx.send(AppEvent::Repos(repos));
                }
            });
        }
        Cmd::LoadMetadata(fingerprint) => {
            tokio::spawn(async move {
                let cache = dnfm_core::MetadataCache::default();
                if let Some(catalog) = cache.load(fingerprint) {
                    tracing::debug!("metadados do cache");
                    let _ = tx.send(AppEvent::Metadata(Box::new(catalog)));
                    return;
                }
                match backend.file_index().await {
                    Ok(index) => {
                        let catalog = dnfm_core::TagCatalog::build_system(index);
                        if let Err(err) = cache.save(&catalog, fingerprint) {
                            tracing::warn!(%err, "não consegui gravar o cache de metadados");
                        }
                        let _ = tx.send(AppEvent::Metadata(Box::new(catalog)));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
            });
        }
        Cmd::LoadHistory => {
            tokio::spawn(async move {
                match backend.history(50).await {
                    Ok(list) => {
                        let _ = tx.send(AppEvent::History(list));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
            });
        }
        Cmd::LoadOrphans => {
            tokio::spawn(async move {
                match backend.orphans().await {
                    Ok(list) => {
                        let _ = tx.send(AppEvent::Orphans(list));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
            });
        }
        Cmd::LoadDetails(nevra) => {
            tokio::spawn(async move {
                if let Ok(details) = backend.details(&nevra).await {
                    let _ = tx.send(AppEvent::Details(details));
                }
            });
        }
        Cmd::Preview(specs) => {
            tokio::spawn(async move {
                match backend.preview_remove(&specs).await {
                    Ok(preview) => {
                        let _ = tx.send(AppEvent::Preview(Box::new(preview)));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
            });
        }
        Cmd::PreviewUpgrade(specs) => {
            tokio::spawn(async move {
                match backend.preview_upgrade(&specs).await {
                    Ok(preview) => {
                        let _ = tx.send(AppEvent::Preview(Box::new(preview)));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
            });
        }
        Cmd::CancelPreview => {
            tokio::spawn(async move {
                let _ = backend.cancel().await;
            });
        }
        Cmd::Execute(token) => {
            tokio::spawn(async move {
                let mut progress = backend.progress();
                let progress_tx = tx.clone();
                let forwarder = tokio::spawn(async move {
                    while let Ok(event) = progress.recv().await {
                        let done = matches!(event, ProgressEvent::Done { .. });
                        let _ = progress_tx.send(AppEvent::Progress(event));
                        if done {
                            break;
                        }
                    }
                });
                match backend.execute(token).await {
                    Ok(outcome) => {
                        let _ = tx.send(AppEvent::Executed(outcome));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Failed(err.short()));
                    }
                }
                forwarder.abort();
            });
        }
    }
}
