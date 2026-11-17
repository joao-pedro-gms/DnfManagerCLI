//! Backend real: fala com o `dnf5daemon-server` por D-Bus.
//!
//! Uma instância mantém uma sessão aberta no daemon. O `Goal` do daemon é
//! estado único da sessão, então preview e execução são serializados.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::{broadcast, Mutex};
use zbus::{OwnedObjectPath, Variant as Value};

use crate::backend::{BackendError, DnfBackend};
use crate::model::{Action, Package, PlannedPackage, Preview, ProgressEvent, Severity, Upgrade};

fn opts(pairs: Vec<(&str, Value<'static>)>) -> HashMap<String, Value<'static>> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn text(value: &str) -> Value<'static> {
    Value::from(value.to_string())
}

fn strings(values: &[&str]) -> Value<'static> {
    let owned: Vec<String> = values.iter().map(|s| s.to_string()).collect();
    Value::from(owned)
}

/// Atributos leves: o suficiente para tabela, busca e ordenação.
const TABLE_ATTRS: &[&str] = &[
    "name",
    "evr",
    "arch",
    "repo_id",
    "from_repo_id",
    "install_size",
    "install_time",
    "summary",
    "reason",
    "nevra",
    "is_installed",
];

mod proxies;

pub(crate) use proxies::*;

pub struct DbusBackend {
    session: OwnedObjectPath,
    rpm: RpmProxy<'static>,
    goal: GoalProxy<'static>,
    advisory: AdvisoryApiProxy<'static>,
    repo: RepoApiProxy<'static>,
    history: HistoryApiProxy<'static>,
    progress: broadcast::Sender<ProgressEvent>,
    gate: Mutex<()>,
    _manager: SessionManagerProxy<'static>,
}

impl DbusBackend {
    pub async fn connect() -> Result<Arc<Self>, BackendError> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|e| BackendError::DaemonUnavailable(e.to_string()))?;

        let manager = SessionManagerProxy::new(&conn)
            .await
            .map_err(|e| BackendError::DaemonUnavailable(e.to_string()))?;

        let session = manager
            .open_session(opts(vec![
                ("load_available_repos", Value::from(true)),
                ("load_system_repo", Value::from(true)),
            ]))
            .await
            .map_err(|e| {
                BackendError::DaemonUnavailable(format!("{e} (tente `{}`)", crate::DAEMON_HINT))
            })?;

        let rpm: RpmProxy<'static> = RpmProxy::builder(&conn)
            .path(session.clone())
            .build()
            .await?;
        let goal = GoalProxy::builder(&conn)
            .path(session.clone())
            .build()
            .await?;
        let advisory = AdvisoryApiProxy::builder(&conn)
            .path(session.clone())
            .build()
            .await?;
        let repo = RepoApiProxy::builder(&conn)
            .path(session.clone())
            .build()
            .await?;
        let history = HistoryApiProxy::builder(&conn)
            .path(session.clone())
            .build()
            .await?;

        let (tx, _) = broadcast::channel(256);
        let backend = Arc::new(Self {
            session,
            rpm,
            goal,
            advisory,
            repo,
            history,
            progress: tx,
            gate: Mutex::new(()),
            _manager: manager,
        });

        backend.spawn_progress_forwarder().await?;
        Ok(backend)
    }

    /// Encaminha os sinais de transação para um canal interno.
    async fn spawn_progress_forwarder(&self) -> Result<(), BackendError> {
        let rpm = self.rpm.clone();
        let tx = self.progress.clone();
        let mut begin = rpm.receive_transaction_before_begin().await?;
        let mut start = rpm.receive_transaction_action_start().await?;
        let mut progress = rpm.receive_transaction_action_progress().await?;
        let mut stop = rpm.receive_transaction_action_stop().await?;
        let mut script = rpm.receive_transaction_script_error().await?;
        let mut done = rpm.receive_transaction_after_complete().await?;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(sig) = begin.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::Begin { total: args.total });
                        }
                    }
                    Some(sig) = start.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::ActionStart {
                                nevra: args.nevra.clone(),
                                action: action_from_code(args.action),
                                total: args.total,
                            });
                        }
                    }
                    Some(sig) = progress.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::ActionProgress {
                                nevra: args.nevra.clone(),
                                processed: args.processed,
                                total: args.total,
                            });
                        }
                    }
                    Some(sig) = stop.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::ActionStop {
                                nevra: args.nevra.clone(),
                                total: args.total,
                            });
                        }
                    }
                    Some(sig) = script.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::ScriptError {
                                nevra: args.nevra.clone(),
                                code: i64::from(args.return_code),
                            });
                        }
                    }
                    Some(sig) = done.next() => {
                        if let Ok(args) = sig.args() {
                            let _ = tx.send(ProgressEvent::Done { success: args.success });
                        }
                    }
                    else => break,
                }
            }
        });
        Ok(())
    }

    async fn list(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> Result<Vec<Package>, BackendError> {
        let raw = self
            .rpm
            .list(options)
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        Ok(raw.iter().map(Package::from_attrs).collect())
    }

    /// Atualizações: junta o lado instalado com o lado novo (`scope=upgrades`).
    async fn collect_upgrades(&self) -> Result<Vec<Upgrade>, BackendError> {
        let installed = self
            .list(opts(vec![
                ("scope", text("upgradable")),
                ("package_attrs", strings(TABLE_ATTRS)),
            ]))
            .await?;
        let candidates = self
            .list(opts(vec![
                ("scope", text("upgrades")),
                ("package_attrs", strings(TABLE_ATTRS)),
            ]))
            .await?;

        let pares = installed
            .into_iter()
            .filter_map(|old| {
                candidates
                    .iter()
                    .find(|candidate| candidate.name == old.name && candidate.arch == old.arch)
                    .cloned()
                    .map(|new| (old, new))
            })
            .collect::<Vec<_>>();

        let out = futures_util::stream::iter(pares)
            .map(|(old, new)| async move {
                let (severity, advisory) =
                    match self.advisories_for(std::slice::from_ref(&old.name)).await {
                        Ok(list) => list
                            .into_iter()
                            .max_by_key(|advisory| advisory.severity)
                            .map(|advisory| (advisory.severity, Some(advisory.id)))
                            .unwrap_or((Severity::Unknown, None)),
                        Err(_) => (Severity::Unknown, None),
                    };
                Upgrade {
                    installed: Package {
                        is_installed: true,
                        ..old
                    },
                    candidate: new,
                    severity,
                    advisory,
                }
            })
            .buffered(8)
            .collect()
            .await;
        Ok(out)
    }
}

fn action_from_code(code: u32) -> Action {
    // Códigos de `libdnf5::transaction::TransactionItemAction`.
    match code {
        0 => Action::Install,
        1 => Action::Upgrade,
        2 => Action::Downgrade,
        3 => Action::Reinstall,
        4 => Action::Remove,
        5 => Action::ReasonChange,
        _ => Action::Unknown,
    }
}

mod backend_impl;

impl From<zbus::Error> for BackendError {
    fn from(err: zbus::Error) -> Self {
        BackendError::Daemon(err.to_string())
    }
}

impl DbusBackend {
    async fn resolve_pending(&self) -> Result<Preview, BackendError> {
        let (items, result) = self
            .goal
            .resolve(opts(vec![]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;

        let planned: Vec<PlannedPackage> = items
            .iter()
            .map(|(_, action, reason, _, attrs)| {
                PlannedPackage::from_transaction(action, reason, attrs)
            })
            .collect();

        let problems = self
            .goal
            .get_transaction_problems_string()
            .await
            .unwrap_or_default();

        // O daemon mantém o goal; um preview aprovado pode ser reutilizado pelo
        // `execute`, então não resetamos aqui.
        Ok(Preview::from_resolve(planned, problems, result))
    }

    pub fn session_path(&self) -> &OwnedObjectPath {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapeia_codigos_de_acao() {
        assert_eq!(action_from_code(0), Action::Install);
        assert_eq!(action_from_code(4), Action::Remove);
        assert_eq!(action_from_code(99), Action::Unknown);
    }
}
