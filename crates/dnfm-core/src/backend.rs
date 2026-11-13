//! Contrato entre a interface e o DNF.
//!
//! Existe para que a TUI e o CLI nunca falem D-Bus diretamente e para que os
//! testes possam rodar contra um backend em memória.

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::model::{
    Advisory, ConfirmToken, HistoryEntry, Outcome, Package, PackageDetails, Preview, ProgressEvent,
    Repo, Upgrade,
};

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("daemon dnf5 indisponível: {0}")]
    DaemonUnavailable(String),
    #[error("autenticação negada pelo polkit")]
    NotAuthorized,
    #[error("o daemon recusou a operação: {0}")]
    Daemon(String),
    #[error("operação bloqueada: {0}")]
    Blocked(String),
    #[error("nenhuma transação preparada; gere um preview antes de executar")]
    NoPreview,
    #[error("tempo esgotado aguardando a transação terminar")]
    Timeout,
    #[error("resposta inesperada do daemon: {0}")]
    Decode(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl BackendError {
    /// Mensagem curta para a barra de status.
    pub fn short(&self) -> String {
        match self {
            Self::DaemonUnavailable(_) => "daemon dnf5 indisponível".to_string(),
            Self::NotAuthorized => "autenticação negada".to_string(),
            Self::Timeout => "transação sem resposta".to_string(),
            other => other.to_string(),
        }
    }
}

/// Fonte de dados de pacotes. Uma instância = uma sessão do daemon.
#[async_trait]
pub trait DnfBackend: Send + Sync {
    /// Pacotes instalados, com os campos leves usados pela tabela.
    async fn installed(&self) -> Result<Vec<Package>, BackendError>;

    /// Atualizações disponíveis (lado novo) pareadas com o instalado.
    async fn upgrades(&self) -> Result<Vec<Upgrade>, BackendError>;

    /// Atualizações de segurança associadas aos nomes informados.
    async fn advisories_for(&self, names: &[String]) -> Result<Vec<Advisory>, BackendError>;

    async fn repos(&self) -> Result<Vec<Repo>, BackendError>;

    async fn history(&self, limit: i64) -> Result<Vec<HistoryEntry>, BackendError>;

    /// Sinais derivados da lista de arquivos de cada pacote instalado.
    ///
    /// É a chamada mais cara do backend (~3,5 s para 8000 pacotes), por isso
    /// deve ser usada com cache e em segundo plano.
    async fn file_index(&self) -> Result<crate::metadata::FileIndex, BackendError>;

    /// Metadados completos de um pacote (para o painel de detalhes).
    async fn details(&self, nevra: &str) -> Result<PackageDetails, BackendError>;

    /// Pacotes instalados como dependência que ninguém mais requer.
    async fn orphans(&self) -> Result<Vec<Package>, BackendError>;

    /// Substitui o goal pendente por uma remoção e resolve.
    async fn preview_remove(&self, specs: &[String]) -> Result<Preview, BackendError>;

    /// Substitui o goal pendente por um upgrade e resolve.
    async fn preview_upgrade(&self, specs: &[String]) -> Result<Preview, BackendError>;

    /// Executa o preview pendente. Exige o token de confirmação.
    async fn execute(&self, token: ConfirmToken) -> Result<Outcome, BackendError>;

    /// Cancela o goal pendente no daemon.
    async fn cancel(&self) -> Result<(), BackendError>;

    fn progress(&self) -> broadcast::Receiver<ProgressEvent>;
}
