//! Filtro, busca e ordenação sobre a lista de pacotes em memória.
//!
//! A lista inteira (8k pacotes) cabe folgada na memória; filtrar é mais rápido
//! do que voltar ao daemon a cada tecla.

use crate::marks::MarkSet;
use crate::model::{Package, Reason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Date,
    Repo,
    Reason,
}

impl SortKey {
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Date,
            Self::Date => Self::Repo,
            Self::Repo => Self::Reason,
            Self::Reason => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "nome",
            Self::Size => "tamanho",
            Self::Date => "data",
            Self::Repo => "repo",
            Self::Reason => "motivo",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Texto livre; casa por subsequência (fuzzy) contra nome e resumo.
    pub text: String,
    pub repo: Option<String>,
    pub reason: Option<Reason>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub marked_only: bool,
    /// `Some(true)` só protegidos, `Some(false)` só não protegidos.
    pub protected: Option<bool>,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        !self.text.is_empty()
            || self.repo.is_some()
            || self.reason.is_some()
            || self.min_size.is_some()
            || self.max_size.is_some()
            || self.marked_only
            || self.protected.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Filtro + ordenação aplicados a um conjunto de pacotes.
pub fn select(
    packages: &[Package],
    filter: &Filter,
    sort: SortKey,
    descending: bool,
    marks: &MarkSet,
) -> Vec<usize> {
    let mut hits: Vec<(usize, i32)> = packages
        .iter()
        .enumerate()
        .filter(|(_, p)| matches(p, filter, marks))
        .map(|(i, p)| {
            let score = if filter.text.is_empty() {
                0
            } else {
                score_package(p, &filter.text).unwrap_or(0)
            };
            (i, score)
        })
        .collect();

    // Com busca ativa, relevância manda; sem busca, o critério escolhido manda.
    if !filter.text.is_empty() {
        hits.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| packages[a.0].name.cmp(&packages[b.0].name))
        });
        if descending {
            hits.reverse();
        }
    } else {
        hits.sort_by(|a, b| {
            let (x, y) = (&packages[a.0], &packages[b.0]);
            let ord = match sort {
                SortKey::Name => x.name.cmp(&y.name),
                SortKey::Size => x.size.cmp(&y.size),
                SortKey::Date => x.install_time.cmp(&y.install_time),
                SortKey::Repo => x.repo_id.cmp(&y.repo_id).then_with(|| x.name.cmp(&y.name)),
                SortKey::Reason => x.reason.cmp(&y.reason).then_with(|| x.name.cmp(&y.name)),
            };
            if descending {
                ord.reverse()
            } else {
                ord
            }
        });
    }

    hits.into_iter().map(|(i, _)| i).collect()
}

fn matches(pkg: &Package, filter: &Filter, marks: &MarkSet) -> bool {
    if let Some(repo) = &filter.repo {
        if &pkg.repo_id != repo && pkg.from_repo_id.as_deref() != Some(repo.as_str()) {
            return false;
        }
    }
    if let Some(reason) = filter.reason {
        if pkg.reason != reason {
            return false;
        }
    }
    if let Some(min) = filter.min_size {
        if pkg.size < min {
            return false;
        }
    }
    if let Some(max) = filter.max_size {
        if pkg.size > max {
            return false;
        }
    }
    if filter.marked_only && !marks.contains(&pkg.name, &pkg.arch) {
        return false;
    }
    if let Some(want) = filter.protected {
        if crate::safety::is_protected(&pkg.name).is_some() != want {
            return false;
        }
    }
    if filter.text.is_empty() || score_package(pkg, &filter.text).is_some() {
        return true;
    }
    false
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reason;

    fn pkg(name: &str, size: u64, reason: Reason, repo: &str) -> Package {
        Package {
            name: name.into(),
            evr: "1-1.fc45".into(),
            arch: "x86_64".into(),
            repo_id: repo.into(),
            from_repo_id: None,
            size,
            install_time: 0,
            summary: format!("{name} summary"),
            reason,
            nevra: format!("{name}-1-1.fc45.x86_64"),
            is_installed: true,
        }
    }

    fn sample() -> Vec<Package> {
        vec![
            pkg("firefox", 300, Reason::User, "@System"),
            pkg("firewalld", 40, Reason::Dependency, "@System"),
            pkg("dosbox-staging", 16, Reason::User, "@System"),
            pkg("vlc", 90, Reason::User, "@System"),
        ]
    }

    #[test]
    fn busca_por_prefixo_ordena_por_relevancia() {
        let pkgs = sample();
        let filter = Filter {
            text: "fire".into(),
            ..Default::default()
        };
        let out = select(&pkgs, &filter, SortKey::Name, false, &MarkSet::empty());
        let names: Vec<&str> = out.iter().map(|i| pkgs[*i].name.as_str()).collect();
        assert_eq!(names[0], "firefox");
        assert!(names.contains(&"firewalld"));
        assert!(!names.contains(&"vlc"));
    }

    #[test]
    fn busca_fuzzy_por_subsequencia() {
        let pkgs = sample();
        let filter = Filter {
            text: "dbx".into(),
            ..Default::default()
        };
        let out = select(&pkgs, &filter, SortKey::Name, false, &MarkSet::empty());
        assert_eq!(out.len(), 1);
        assert_eq!(pkgs[out[0]].name, "dosbox-staging");
    }

    #[test]
    fn ordena_por_tamanho_decrescente() {
        let pkgs = sample();
        let out = select(
            &pkgs,
            &Filter::default(),
            SortKey::Size,
            true,
            &MarkSet::empty(),
        );
        let sizes: Vec<u64> = out.iter().map(|i| pkgs[*i].size).collect();
        assert_eq!(sizes, vec![300, 90, 40, 16]);
    }

    #[test]
    fn filtra_por_motivo() {
        let pkgs = sample();
        let filter = Filter {
            reason: Some(Reason::Dependency),
            ..Default::default()
        };
        let out = select(&pkgs, &filter, SortKey::Name, false, &MarkSet::empty());
        assert_eq!(out.len(), 1);
        assert_eq!(pkgs[out[0]].name, "firewalld");
    }

    #[test]
    fn sem_filtro_devolve_tudo_sem_duplicata() {
        let pkgs = sample();
        let out = select(
            &pkgs,
            &Filter::default(),
            SortKey::Name,
            false,
            &MarkSet::empty(),
        );
        assert_eq!(out.len(), pkgs.len());
        let mut sorted = out.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), pkgs.len());
    }
}
