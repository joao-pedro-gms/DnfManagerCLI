use super::{DatePreset, SizePreset};
use crate::model::{Package, Reason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Date,
    Repo,
    Reason,
    Category,
    Family,
    Arch,
}

impl SortKey {
    pub const ALL: [SortKey; 8] = [
        SortKey::Name,
        SortKey::Size,
        SortKey::Date,
        SortKey::Repo,
        SortKey::Reason,
        SortKey::Category,
        SortKey::Family,
        SortKey::Arch,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|k| *k == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "nome",
            Self::Size => "tamanho",
            Self::Date => "data",
            Self::Repo => "repo",
            Self::Reason => "motivo",
            Self::Category => "categoria",
            Self::Family => "família",
            Self::Arch => "arch",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Date => "date",
            Self::Repo => "repo",
            Self::Reason => "reason",
            Self::Category => "category",
            Self::Family => "family",
            Self::Arch => "arch",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "name" | "nome" => Some(Self::Name),
            "size" | "tamanho" => Some(Self::Size),
            "date" | "data" => Some(Self::Date),
            "repo" => Some(Self::Repo),
            "reason" | "motivo" => Some(Self::Reason),
            "category" | "categoria" => Some(Self::Category),
            "family" | "familia" | "família" => Some(Self::Family),
            "arch" | "arquitetura" => Some(Self::Arch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Texto livre; casa por subsequência (fuzzy) contra nome e resumo.
    pub text: String,
    pub repo: Option<String>,
    pub reason: Option<Reason>,
    pub arch: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub installed_since: Option<i64>,
    pub marked_only: bool,
    /// `Some(true)` só protegidos, `Some(false)` só não protegidos.
    pub protected: Option<bool>,
    /// Categorias aceitas (vazio = todas).
    pub categories: Vec<String>,
    /// Famílias aceitas (vazio = todas).
    pub families: Vec<String>,
    /// Só pacotes com atualização disponível.
    pub only_updates: bool,
    pub size_preset: SizePreset,
    pub date_preset: DatePreset,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        self.active_count() > 0
    }

    /// Quantos filtros estão ativos, para mostrar na barra de status.
    pub fn active_count(&self) -> usize {
        let mut n = 0;
        n += usize::from(!self.text.is_empty());
        n += usize::from(self.repo.is_some());
        n += usize::from(self.reason.is_some());
        n += usize::from(self.arch.is_some());
        n += usize::from(self.min_size.is_some() || self.max_size.is_some());
        n += usize::from(self.installed_since.is_some());
        n += usize::from(self.marked_only);
        n += usize::from(self.protected.is_some());
        n += usize::from(!self.categories.is_empty());
        n += usize::from(!self.families.is_empty());
        n += usize::from(self.only_updates);
        n
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn apply_size_preset(&mut self, preset: SizePreset) {
        self.size_preset = preset;
        let (min, max) = preset.bounds();
        self.min_size = min;
        self.max_size = max;
    }

    pub fn apply_date_preset(&mut self, preset: DatePreset, now: i64) {
        self.date_preset = preset;
        self.installed_since = preset.since(now);
    }

    pub fn toggle_category(&mut self, label: &str) {
        if let Some(pos) = self.categories.iter().position(|c| c == label) {
            self.categories.remove(pos);
        } else {
            self.categories.push(label.to_string());
        }
    }

    pub fn toggle_family(&mut self, label: &str) {
        if let Some(pos) = self.families.iter().position(|f| f == label) {
            self.families.remove(pos);
        } else {
            self.families.push(label.to_string());
        }
    }
}

/// Pontuação de busca: prefixo > substring > subsequência.
pub fn score_package(pkg: &Package, query: &str) -> Option<i32> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let name = pkg.name.to_ascii_lowercase();
    if name == q {
        return Some(1000);
    }
    if name.starts_with(&q) {
        return Some(800 - name.len() as i32);
    }
    if let Some(pos) = name.find(&q) {
        return Some(600 - pos as i32);
    }
    if let Some(s) = subsequence_score(&name, &q) {
        return Some(s);
    }
    let summary = pkg.summary.to_ascii_lowercase();
    if summary.contains(&q) {
        return Some(200);
    }
    subsequence_score(&summary, &q).map(|s| s / 4)
}

/// Casamento por subsequência: `dbx` casa com `dosbox-staging`.
fn subsequence_score(haystack: &str, needle: &str) -> Option<i32> {
    let mut hay = haystack.chars().enumerate();
    let mut score = 0i32;
    let mut last: Option<usize> = None;
    for want in needle.chars() {
        loop {
            let (idx, ch) = hay.next()?;
            if ch == want {
                // Bônus por casar no início de palavra e por casar seguido.
                if idx == 0 || haystack.as_bytes().get(idx - 1) == Some(&b'-') {
                    score += 12;
                }
                if let Some(prev) = last {
                    if idx == prev + 1 {
                        score += 6;
                    }
                }
                score += 8;
                last = Some(idx);
                break;
            }
        }
    }
    Some(score)
}
