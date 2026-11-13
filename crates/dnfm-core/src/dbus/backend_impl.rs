//! Implementação do contrato [`DnfBackend`] sobre a sessão do daemon.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;
use zbus::Variant as Value;

use crate::attrs::AttrReader;
use crate::backend::{BackendError, DnfBackend};
use crate::model::{
    Advisory, ConfirmToken, HistoryEntry, Outcome, Package, PackageDetails, Preview, ProgressEvent,
    Repo, Upgrade,
};

use super::{opts, strings, text, DbusBackend, TABLE_ATTRS};

#[async_trait]
impl DnfBackend for DbusBackend {
    async fn installed(&self) -> Result<Vec<Package>, BackendError> {
        let mut packages = self
            .list(opts(vec![
                ("scope", text("installed")),
                ("package_attrs", strings(TABLE_ATTRS)),
            ]))
            .await?;
        for pkg in &mut packages {
            pkg.is_installed = true;
        }
        Ok(packages)
    }

    async fn upgrades(&self) -> Result<Vec<Upgrade>, BackendError> {
        self.collect_upgrades().await
    }

    async fn advisories_for(&self, names: &[String]) -> Result<Vec<Advisory>, BackendError> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let raw = self
            .advisory
            .list(opts(vec![
                (
                    "advisory_attrs",
                    strings(&["advisoryid", "type", "severity", "title"]),
                ),
                ("contains_pkgs", strings(&refs)),
                ("availability", text("available")),
            ]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        let mut seen = HashSet::new();
        Ok(raw
            .iter()
            .map(Advisory::from_attrs)
            .filter(|a| !a.id.is_empty() && seen.insert(a.id.clone()))
            .collect())
    }

    async fn repos(&self) -> Result<Vec<Repo>, BackendError> {
        let raw = self
            .repo
            .list(opts(vec![(
                "repo_attrs",
                strings(&["id", "name", "enabled"]),
            )]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        let mut repos: Vec<Repo> = raw
            .iter()
            .map(Repo::from_attrs)
            .filter(|r| !r.id.is_empty())
            .collect();
        repos.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(repos)
    }

    async fn history(&self, limit: i64) -> Result<Vec<HistoryEntry>, BackendError> {
        let raw = self
            .history
            .list(opts(vec![
                ("limit", Value::from(limit)),
                ("package_attrs", strings(&["name", "evr", "arch"])),
            ]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        Ok(raw.iter().map(HistoryEntry::from_attrs).collect())
    }

    async fn file_index(&self) -> Result<crate::metadata::FileIndex, BackendError> {
        let raw = self
            .rpm
            .list(opts(vec![
                ("scope", text("installed")),
                ("package_attrs", strings(&["name", "files", "url"])),
            ]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;

        let mut index = crate::metadata::FileIndex::default();
        for attrs in &raw {
            let name = attrs.str_or("name", "");
            if name.is_empty() {
                continue;
            }
            let caminhos = attrs.get_str_list("files").unwrap_or_default();
            index
                .files
                .insert(name.clone(), crate::metadata::signals_from_paths(caminhos));
            if let Some(familia) = crate::metadata::family_from_url(&attrs.str_or("url", "")) {
                index.families.insert(name, familia.to_string());
            }
        }
        Ok(index)
    }

    async fn details(&self, nevra: &str) -> Result<PackageDetails, BackendError> {
        let raw = self
            .rpm
            .list(opts(vec![
                ("scope", text("all")),
                ("patterns", strings(&[nevra])),
                (
                    "package_attrs",
                    strings(&["nevra", "url", "license", "description", "vendor", "group"]),
                ),
            ]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        raw.first()
            .map(PackageDetails::from_attrs)
            .ok_or_else(|| BackendError::Decode(format!("pacote {nevra} não encontrado")))
    }

    async fn orphans(&self) -> Result<Vec<Package>, BackendError> {
        let raw = self
            .rpm
            .list(opts(vec![
                ("scope", text("installed")),
                (
                    "package_attrs",
                    strings(&[
                        "name",
                        "arch",
                        "repo_id",
                        "install_size",
                        "summary",
                        "reason",
                        "nevra",
                        "requires",
                        "provides",
                    ]),
                ),
            ]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;

        // capacidade -> pacotes que a oferecem
        let mut providers: HashMap<String, HashSet<String>> = HashMap::new();
        for attrs in &raw {
            let name = attrs.str_or("name", "");
            if let Some(provides) = attrs.get_str_list("provides") {
                for capability in provides {
                    providers
                        .entry(capability)
                        .or_default()
                        .insert(name.clone());
                }
            }
        }

        let mut required: HashSet<String> = HashSet::new();
        for attrs in &raw {
            let name = attrs.str_or("name", "");
            if let Some(requires) = attrs.get_str_list("requires") {
                for capability in requires {
                    if let Some(owners) = providers.get(&capability) {
                        for owner in owners {
                            if owner != &name {
                                required.insert(owner.clone());
                            }
                        }
                    }
                }
            }
        }

        let mut out = Vec::new();
        for attrs in &raw {
            let name = attrs.str_or("name", "");
            if name.is_empty() || required.contains(&name) {
                continue;
            }
            let mut pkg = Package::from_attrs(attrs);
            pkg.is_installed = true;
            // Só faz sentido sugerir remoção do que entrou como dependência.
            if !pkg.reason.is_user() {
                out.push(pkg);
            }
        }
        out.sort_by_key(|p| std::cmp::Reverse(p.size));
        Ok(out)
    }

    async fn preview_remove(&self, specs: &[String]) -> Result<Preview, BackendError> {
        let _guard = self.gate.lock().await;
        self.goal
            .reset()
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        self.rpm
            .remove(specs, opts(vec![("interactive", Value::from(true))]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        self.resolve_pending().await
    }

    async fn preview_upgrade(&self, specs: &[String]) -> Result<Preview, BackendError> {
        let _guard = self.gate.lock().await;
        self.goal
            .reset()
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        self.rpm
            .upgrade(specs, opts(vec![("interactive", Value::from(true))]))
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))?;
        self.resolve_pending().await
    }

    async fn execute(&self, token: ConfirmToken) -> Result<Outcome, BackendError> {
        if token.is_empty() {
            return Err(BackendError::NoPreview);
        }
        let _guard = self.gate.lock().await;
        let mut rx = self.progress.subscribe();
        self.goal
            .do_transaction(opts(vec![("interactive", Value::from(true))]))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("NotAuthorized") || msg.contains("not authorized") {
                    BackendError::NotAuthorized
                } else {
                    BackendError::Daemon(msg)
                }
            })?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(60 * 30);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(BackendError::Timeout);
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(ProgressEvent::Done { success })) => return Ok(Outcome { success }),
                Ok(Ok(_)) => continue,
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(_)) => return Err(BackendError::Timeout),
                Err(_) => return Err(BackendError::Timeout),
            }
        }
    }

    async fn cancel(&self) -> Result<(), BackendError> {
        self.goal
            .reset()
            .await
            .map_err(|e| BackendError::Daemon(e.to_string()))
    }

    fn progress(&self) -> broadcast::Receiver<ProgressEvent> {
        self.progress.subscribe()
    }
}
