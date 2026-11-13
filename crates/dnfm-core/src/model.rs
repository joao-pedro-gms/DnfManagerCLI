//! Tipos de domínio do dnfm.
//!
//! Espelham o que o `dnf5daemon` devolve em `aa{sv}`; a conversão vive em
//! [`crate::backend::dbus`].

use std::fmt;

/// Por que um pacote está instalado, na visão do DNF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reason {
    User,
    Group,
    WeakDependency,
    Dependency,
    Clean,
    Unknown,
}

impl Reason {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "User" => Self::User,
            "Group" => Self::Group,
            "Weak Dependency" => Self::WeakDependency,
            "Dependency" => Self::Dependency,
            "Clean" => Self::Clean,
            _ => Self::Unknown,
        }
    }

    /// Rótulo curto usado na interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "usuário",
            Self::Group => "grupo",
            Self::WeakDependency => "dep. fraca",
            Self::Dependency => "dependência",
            Self::Clean => "limpeza",
            Self::Unknown => "desconhecido",
        }
    }

    pub fn is_user(self) -> bool {
        self == Self::User
    }

    /// Identificador estável para JSON e scripts (não traduzir).
    pub fn key(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Group => "group",
            Self::WeakDependency => "weak_dependency",
            Self::Dependency => "dependency",
            Self::Clean => "clean",
            Self::Unknown => "unknown",
        }
    }
}

/// Ação que a transação vai executar sobre um pacote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Install,
    Upgrade,
    Downgrade,
    Reinstall,
    Remove,
    ReasonChange,
    Obsoleted,
    Unknown,
}

impl Action {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "Install" => Self::Install,
            "Upgrade" => Self::Upgrade,
            "Downgrade" => Self::Downgrade,
            "Reinstall" => Self::Reinstall,
            "Remove" => Self::Remove,
            "Reason Change" => Self::ReasonChange,
            "Obsoleted" => Self::Obsoleted,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Install => "instalar",
            Self::Upgrade => "atualizar",
            Self::Downgrade => "rebaixar",
            Self::Reinstall => "reinstalar",
            Self::Remove => "remover",
            Self::ReasonChange => "motivo",
            Self::Obsoleted => "obsoleto",
            Self::Unknown => "?",
        }
    }

    pub fn is_removal(self) -> bool {
        self == Self::Remove
    }
}

/// Um pacote RPM, instalado ou disponível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub evr: String,
    pub arch: String,
    pub repo_id: String,
    pub from_repo_id: Option<String>,
    pub size: u64,
    pub install_time: i64,
    pub summary: String,
    pub reason: Reason,
    pub nevra: String,
    pub is_installed: bool,
}

impl Package {
    /// Chave estável entre versões: nome + arquitetura.
    pub fn key(&self) -> String {
        format!("{}.{}", self.name, self.arch)
    }

    pub fn summary_or_empty(&self) -> &str {
        if self.summary.is_empty() {
            "—"
        } else {
            &self.summary
        }
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.nevra.is_empty() {
            write!(f, "{}-{}.{}", self.name, self.evr, self.arch)
        } else {
            f.write_str(&self.nevra)
        }
    }
}

/// Detalhes carregados sob demanda para o painel inferior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageDetails {
    pub nevra: String,
    pub url: String,
    pub license: String,
    pub description: String,
    pub vendor: String,
    pub group: String,
}

/// Severidade de um advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Unknown,
    Low,
    Moderate,
    Important,
    Critical,
}

impl Severity {
    pub fn parse(raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "low" => Self::Low,
            "moderate" => Self::Moderate,
            "important" => Self::Important,
            "critical" => Self::Critical,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "—",
            Self::Low => "baixa",
            Self::Moderate => "moderada",
            Self::Important => "importante",
            Self::Critical => "crítica",
        }
    }

    pub fn is_security_relevant(self) -> bool {
        matches!(self, Self::Important | Self::Critical)
    }

    /// Identificador estável para JSON e scripts.
    pub fn key(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::Important => "important",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    pub id: String,
    pub kind: String,
    pub severity: Severity,
    pub title: String,
}

/// Pacote instalado com atualização disponível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upgrade {
    pub installed: Package,
    pub candidate: Package,
    pub severity: Severity,
    pub advisory: Option<String>,
}

impl Upgrade {
    pub fn name(&self) -> &str {
        &self.installed.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub description: String,
    pub status: String,
    pub start: i64,
    pub end: i64,
    pub user_id: i64,
    pub installed: Vec<String>,
    pub removed: Vec<String>,
    pub upgraded: Vec<String>,
    pub downgraded: Vec<String>,
}

impl HistoryEntry {
    pub fn changes(&self) -> usize {
        self.installed.len() + self.removed.len() + self.upgraded.len() + self.downgraded.len()
    }
}

/// Um pacote dentro de uma transação planejada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPackage {
    pub action: Action,
    pub reason: Reason,
    pub package: Package,
}

/// Resultado do resolvedor do DNF.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResolveResult {
    Ok,
    Warnings,
    #[default]
    Failed,
}

impl ResolveResult {
    pub fn parse(raw: u32) -> Self {
        match raw {
            0 => Self::Ok,
            1 => Self::Warnings,
            _ => Self::Failed,
        }
    }
}

/// Prévia de uma transação: o que sai, o que entra e o que quebra.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    pub items: Vec<PlannedPackage>,
    pub problems: Vec<String>,
    pub result: ResolveResult,
}

impl Preview {
    pub fn removals(&self) -> impl Iterator<Item = &PlannedPackage> {
        self.items.iter().filter(|i| i.action.is_removal())
    }

    pub fn additions(&self) -> impl Iterator<Item = &PlannedPackage> {
        self.items.iter().filter(|i| !i.action.is_removal())
    }

    /// Pacotes que o usuário pediu explicitamente.
    pub fn requested(&self) -> Vec<&PlannedPackage> {
        self.removals().filter(|i| i.reason.is_user()).collect()
    }

    /// Pacotes arrastados junto (dependência, dependência fraca, limpeza).
    pub fn collateral(&self) -> Vec<&PlannedPackage> {
        self.removals().filter(|i| !i.reason.is_user()).collect()
    }

    pub fn freed_bytes(&self) -> u64 {
        self.removals().map(|i| i.package.size).sum()
    }

    pub fn downloaded_bytes(&self) -> u64 {
        self.additions().map(|i| i.package.size).sum()
    }

    pub fn is_failed(&self) -> bool {
        self.result == ResolveResult::Failed
    }

    /// Converte a prévia em autorização de execução.
    ///
    /// É a única forma de obter um [`ConfirmToken`], então não existe caminho
    /// de código que execute uma transação sem ter passado por um preview.
    pub fn confirm(self) -> Result<ConfirmToken, crate::BackendError> {
        if self.is_failed() {
            return Err(crate::BackendError::Blocked(if self.problems.is_empty() {
                "resolução da transação falhou".to_string()
            } else {
                self.problems.join("; ")
            }));
        }
        if self.items.is_empty() {
            return Err(crate::BackendError::Blocked(
                "a transação não alteraria nenhum pacote".to_string(),
            ));
        }
        Ok(ConfirmToken {
            removals: self.removals().count(),
            additions: self.additions().count(),
            _sealed: (),
        })
    }
}

/// Prova de que um preview aprovado existe. Não pode ser construído à mão.
#[derive(Debug, Clone)]
pub struct ConfirmToken {
    removals: usize,
    additions: usize,
    _sealed: (),
}

impl ConfirmToken {
    pub fn removals(&self) -> usize {
        self.removals
    }

    pub fn additions(&self) -> usize {
        self.additions
    }

    pub fn is_empty(&self) -> bool {
        self.removals == 0 && self.additions == 0
    }
}

/// Eventos emitidos durante uma transação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    Begin {
        total: u64,
    },
    Download {
        description: String,
        downloaded: u64,
        total: u64,
    },
    ActionStart {
        nevra: String,
        action: Action,
        total: u64,
    },
    ActionProgress {
        nevra: String,
        processed: u64,
        total: u64,
    },
    ActionStop {
        nevra: String,
        total: u64,
    },
    ScriptError {
        nevra: String,
        code: i64,
    },
    Done {
        success: bool,
    },
}

/// Desfecho de uma transação executada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub success: bool,
}

/// Formata bytes de forma legível (base 1024).
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Formata um timestamp unix como `YYYY-MM-DD` (UTC, sem dependência de crate).
pub fn format_date(unix: i64) -> String {
    if unix <= 0 {
        return "—".to_string();
    }
    let days = unix.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Algoritmo de Howard Hinnant: dias desde 1970 -> (ano, mês, dia).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
