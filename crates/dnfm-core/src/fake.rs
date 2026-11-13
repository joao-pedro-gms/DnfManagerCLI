//! Backend em memória para testes e para `dnfm --offline`.
//!
//! Reproduz os cenários que importam sem tocar no sistema: transação ok,
//! resolução bloqueada, polkit negado e scriptlet com erro.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::backend::{BackendError, DnfBackend};
use crate::model::{
    Action, Advisory, ConfirmToken, HistoryEntry, Outcome, Package, PackageDetails, PlannedPackage,
    Preview, ProgressEvent, Reason, Repo, ResolveResult, Severity, Upgrade,
};

fn pkg(name: &str, size: u64, reason: Reason) -> Package {
    Package {
        name: name.to_string(),
        evr: "1.0-1.fc45".to_string(),
        arch: "x86_64".to_string(),
        repo_id: "@System".to_string(),
        from_repo_id: None,
        size,
        install_time: 1_793_727_256,
        summary: format!("{name} (fixture)"),
        reason,
        nevra: format!("{name}-1.0-1.fc45.x86_64"),
        is_installed: true,
    }
}

#[derive(Debug, Clone)]
pub struct FakeBackend {
    pub installed_packages: Vec<Package>,
    pub upgrade_list: Vec<Upgrade>,
    pub preview_remove_result: Preview,
    pub preview_upgrade_result: Preview,
    pub repo_list: Vec<Repo>,
    pub history_list: Vec<HistoryEntry>,
    pub advisory_list: Vec<Advisory>,
    pub orphan_list: Vec<Package>,
    pub fail_execution: bool,
    pub deny_authorization: bool,
    progress: broadcast::Sender<ProgressEvent>,
}

impl Default for FakeBackend {
    fn default() -> Self {
        let installed = vec![
            pkg("vlc", 90 * 1024 * 1024, Reason::User),
            pkg("firefox", 312 * 1024 * 1024, Reason::User),
            pkg("dosbox-staging", 16 * 1024 * 1024, Reason::User),
            pkg("SDL2_net", 27 * 1024, Reason::Dependency),
            pkg("iir1", 49 * 1024, Reason::Dependency),
            pkg("glibc", 12 * 1024 * 1024, Reason::Dependency),
        ];
        let preview = Preview {
            items: vec![
                PlannedPackage {
                    action: Action::Remove,
                    reason: Reason::User,
                    package: installed[2].clone(),
                },
                PlannedPackage {
                    action: Action::Remove,
                    reason: Reason::Dependency,
                    package: installed[3].clone(),
                },
                PlannedPackage {
                    action: Action::Remove,
                    reason: Reason::Dependency,
                    package: installed[4].clone(),
                },
            ],
            problems: Vec::new(),
            result: ResolveResult::Ok,
        };
        let (tx, _) = broadcast::channel(64);
        Self {
            installed_packages: installed,
            upgrade_list: vec![Upgrade {
                installed: pkg("firefox", 312 * 1024 * 1024, Reason::User),
                candidate: Package {
                    evr: "160.0-1.fc45".to_string(),
                    nevra: "firefox-160.0-1.fc45.x86_64".to_string(),
                    repo_id: "updates".to_string(),
                    is_installed: false,
                    ..pkg("firefox", 315 * 1024 * 1024, Reason::User)
                },
                severity: Severity::Important,
                advisory: Some("FEDORA-2026-abc".to_string()),
            }],
            preview_remove_result: preview,
            preview_upgrade_result: Preview::default(),
            repo_list: vec![
                Repo {
                    id: "fedora".into(),
                    name: "Fedora 45 - x86_64".into(),
                    enabled: true,
                },
                Repo {
                    id: "updates".into(),
                    name: "Fedora 45 - x86_64 - Updates".into(),
                    enabled: true,
                },
            ],
            history_list: vec![HistoryEntry {
                id: 99,
                description: "dnf remove warp-terminal -y".to_string(),
                status: "Ok".to_string(),
                start: 1_794_567_129,
                end: 1_794_567_130,
                user_id: 1000,
                installed: Vec::new(),
                removed: vec!["warp-terminal".to_string()],
                upgraded: Vec::new(),
                downgraded: Vec::new(),
            }],
            advisory_list: vec![Advisory {
                id: "FEDORA-2026-abc".into(),
                kind: "security".into(),
                severity: Severity::Critical,
                title: "firefox-160.0-1.fc45".into(),
            }],
            orphan_list: vec![pkg("SDL2_net", 27 * 1024, Reason::Dependency)],
            fail_execution: false,
            deny_authorization: false,
            progress: tx,
        }
    }
}

impl FakeBackend {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Preview que o resolvedor recusou (equivalente a remover `glibc`).
    pub fn blocked() -> Arc<Self> {
        Arc::new(Self {
            preview_remove_result: Preview {
                items: Vec::new(),
                problems: vec![
                    "Problem: installed package linuxtoys requires rtld(GNU_HASH)".to_string(),
                ],
                result: ResolveResult::Failed,
            },
            ..Self::default()
        })
    }

    /// Simula polkit negado na hora de executar.
    pub fn denied() -> Arc<Self> {
        Arc::new(Self {
            deny_authorization: true,
            ..Self::default()
        })
    }
}

#[async_trait]
impl DnfBackend for FakeBackend {
    async fn installed(&self) -> Result<Vec<Package>, BackendError> {
        Ok(self.installed_packages.clone())
    }

    async fn upgrades(&self) -> Result<Vec<Upgrade>, BackendError> {
        Ok(self.upgrade_list.clone())
    }

    async fn advisories_for(&self, _names: &[String]) -> Result<Vec<Advisory>, BackendError> {
        Ok(self.advisory_list.clone())
    }

    async fn repos(&self) -> Result<Vec<Repo>, BackendError> {
        Ok(self.repo_list.clone())
    }

    async fn history(&self, _limit: i64) -> Result<Vec<HistoryEntry>, BackendError> {
        Ok(self.history_list.clone())
    }

    async fn file_index(&self) -> Result<crate::metadata::FileIndex, BackendError> {
        let mut out = crate::metadata::FileIndex::default();
        for (idx, pkg) in self.installed_packages.iter().enumerate() {
            let caminhos: Vec<String> = match idx % 3 {
                0 => vec![
                    format!("/usr/bin/{}", pkg.name),
                    format!("/usr/share/man/man1/{}.1.gz", pkg.name),
                ],
                1 => vec![format!("/usr/lib64/lib{}.so.1", pkg.name)],
                _ => vec![
                    format!("/usr/share/applications/{}.desktop", pkg.name),
                    format!("/usr/bin/{}", pkg.name),
                ],
            };
            out.files.insert(
                pkg.name.clone(),
                crate::metadata::signals_from_paths(caminhos),
            );
        }
        out.families
            .insert("vlc".to_string(), "Multimídia".to_string());
        Ok(out)
    }

    async fn details(&self, nevra: &str) -> Result<PackageDetails, BackendError> {
        Ok(PackageDetails {
            nevra: nevra.to_string(),
            url: "https://example.invalid".to_string(),
            license: "GPL-2.0-or-later".to_string(),
            description: "Pacote de exemplo do backend fake.".to_string(),
            vendor: "Fedora Project".to_string(),
            group: "Applications/System".to_string(),
        })
    }

    async fn orphans(&self) -> Result<Vec<Package>, BackendError> {
        Ok(self.orphan_list.clone())
    }

    async fn preview_remove(&self, _specs: &[String]) -> Result<Preview, BackendError> {
        Ok(self.preview_remove_result.clone())
    }

    async fn preview_upgrade(&self, _specs: &[String]) -> Result<Preview, BackendError> {
        Ok(self.preview_upgrade_result.clone())
    }

    async fn execute(&self, token: ConfirmToken) -> Result<Outcome, BackendError> {
        if token.is_empty() {
            return Err(BackendError::NoPreview);
        }
        if self.deny_authorization {
            return Err(BackendError::NotAuthorized);
        }
        let total = token.removals() as u64;
        let _ = self.progress.send(ProgressEvent::Begin { total });
        for item in &self.preview_remove_result.items {
            let package = item.package.clone();
            let _ = self.progress.send(ProgressEvent::ActionStart {
                nevra: package.nevra.clone(),
                action: item.action,
                total,
            });
            let _ = self.progress.send(ProgressEvent::ActionStop {
                nevra: package.nevra.clone(),
                total,
            });
        }
        let success = !self.fail_execution;
        let _ = self.progress.send(ProgressEvent::Done { success });
        Ok(Outcome { success })
    }

    async fn cancel(&self) -> Result<(), BackendError> {
        Ok(())
    }

    fn progress(&self) -> broadcast::Receiver<ProgressEvent> {
        self.progress.subscribe()
    }
}
