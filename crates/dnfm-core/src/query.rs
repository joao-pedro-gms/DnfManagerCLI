//! Filtro, busca e ordenação sobre a lista de pacotes em memória.
//!
//! A lista inteira (8k pacotes) cabe folgada na memória; filtrar é mais rápido
//! do que voltar ao daemon a cada tecla.

use std::collections::HashSet;

use crate::categories::Taxonomy;
use crate::marks::MarkSet;
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

/// Presets de tamanho usados na interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SizePreset {
    #[default]
    Any,
    Over100MiB,
    Over10MiB,
    Over1MiB,
    Under1MiB,
}

impl SizePreset {
    pub const ALL: [SizePreset; 5] = [
        SizePreset::Any,
        SizePreset::Over100MiB,
        SizePreset::Over10MiB,
        SizePreset::Over1MiB,
        SizePreset::Under1MiB,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "qualquer",
            Self::Over100MiB => "> 100 MiB",
            Self::Over10MiB => "> 10 MiB",
            Self::Over1MiB => "> 1 MiB",
            Self::Under1MiB => "< 1 MiB",
        }
    }

    pub fn bounds(self) -> (Option<u64>, Option<u64>) {
        const MIB: u64 = 1024 * 1024;
        match self {
            Self::Any => (None, None),
            Self::Over100MiB => (Some(100 * MIB), None),
            Self::Over10MiB => (Some(10 * MIB), None),
            Self::Over1MiB => (Some(MIB), None),
            Self::Under1MiB => (None, Some(MIB - 1)),
        }
    }
}

/// Presets de data de instalação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DatePreset {
    #[default]
    Any,
    Last7Days,
    Last30Days,
    Last90Days,
    ThisYear,
}

impl DatePreset {
    pub const ALL: [DatePreset; 5] = [
        DatePreset::Any,
        DatePreset::Last7Days,
        DatePreset::Last30Days,
        DatePreset::Last90Days,
        DatePreset::ThisYear,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "qualquer",
            Self::Last7Days => "últimos 7 dias",
            Self::Last30Days => "últimos 30 dias",
            Self::Last90Days => "últimos 90 dias",
            Self::ThisYear => "este ano",
        }
    }

    /// Início do intervalo em timestamp unix, relativo a `now`.
    pub fn since(self, now: i64) -> Option<i64> {
        const DAY: i64 = 86_400;
        match self {
            Self::Any => None,
            Self::Last7Days => Some(now - 7 * DAY),
            Self::Last30Days => Some(now - 30 * DAY),
            Self::Last90Days => Some(now - 90 * DAY),
            Self::ThisYear => {
                // Aproximação em blocos de 365 dias: suficiente para um preset.
                let years = now.div_euclid(DAY) / 365;
                Some(years * 365 * DAY)
            }
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

/// Contexto de consulta: tudo que o filtro precisa além dos pacotes.
pub struct Query<'a> {
    pub filter: &'a Filter,
    pub sort: SortKey,
    pub descending: bool,
    pub marks: &'a MarkSet,
    pub taxonomy: &'a Taxonomy,
    pub upgradable: &'a HashSet<String>,
}

impl Query<'_> {
    pub fn apply(&self, packages: &[Package]) -> Vec<usize> {
        let mut hits: Vec<(usize, i32)> = packages
            .iter()
            .enumerate()
            .filter(|(_, p)| self.matches(p))
            .map(|(i, p)| {
                let score = if self.filter.text.is_empty() {
                    0
                } else {
                    score_package(p, &self.filter.text).unwrap_or(0)
                };
                (i, score)
            })
            .collect();

        // Com busca ativa, relevância manda; sem busca, o critério escolhido manda.
        if !self.filter.text.is_empty() {
            hits.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| packages[a.0].name.cmp(&packages[b.0].name))
            });
            if self.descending {
                hits.reverse();
            }
        } else {
            let tax = self.taxonomy;
            hits.sort_by(|a, b| {
                let (x, y) = (&packages[a.0], &packages[b.0]);
                let ord = match self.sort {
                    SortKey::Name => x.name.cmp(&y.name),
                    SortKey::Size => x.size.cmp(&y.size),
                    SortKey::Date => x.install_time.cmp(&y.install_time),
                    SortKey::Repo => x.repo_id.cmp(&y.repo_id).then_with(|| x.name.cmp(&y.name)),
                    SortKey::Reason => x.reason.cmp(&y.reason).then_with(|| x.name.cmp(&y.name)),
                    SortKey::Category => tax
                        .category_of(x)
                        .cmp(tax.category_of(y))
                        .then_with(|| x.name.cmp(&y.name)),
                    SortKey::Family => tax
                        .family_of(x)
                        .cmp(tax.family_of(y))
                        .then_with(|| x.name.cmp(&y.name)),
                    SortKey::Arch => x.arch.cmp(&y.arch).then_with(|| x.name.cmp(&y.name)),
                };
                if self.descending {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }

        hits.into_iter().map(|(i, _)| i).collect()
    }

    fn matches(&self, pkg: &Package) -> bool {
        let filter = self.filter;
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
        if let Some(arch) = &filter.arch {
            if &pkg.arch != arch {
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
        if let Some(since) = filter.installed_since {
            if pkg.install_time != 0 && pkg.install_time < since {
                return false;
            }
        }
        if filter.marked_only && !self.marks.contains(&pkg.name, &pkg.arch) {
            return false;
        }
        if let Some(want) = filter.protected {
            if crate::safety::is_protected(&pkg.name).is_some() != want {
                return false;
            }
        }
        if !filter.categories.is_empty()
            && !filter
                .categories
                .iter()
                .any(|c| c == self.taxonomy.category_of(pkg))
        {
            return false;
        }
        if !filter.families.is_empty()
            && !filter
                .families
                .iter()
                .any(|f| f == self.taxonomy.family_of(pkg))
        {
            return false;
        }
        if filter.only_updates && !self.upgradable.contains(&pkg.name) {
            return false;
        }
        if filter.text.is_empty() || score_package(pkg, &filter.text).is_some() {
            return true;
        }
        false
    }
}

/// Atalho para consultas sem categorias nem atualizações.
pub fn select(
    packages: &[Package],
    filter: &Filter,
    sort: SortKey,
    descending: bool,
    marks: &MarkSet,
) -> Vec<usize> {
    let taxonomy = Taxonomy::default();
    let upgradable = HashSet::new();
    Query {
        filter,
        sort,
        descending,
        marks,
        taxonomy: &taxonomy,
        upgradable: &upgradable,
    }
    .apply(packages)
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
    use crate::categories::CustomCategories;
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

    fn names(pkgs: &[Package], out: &[usize]) -> Vec<String> {
        out.iter().map(|i| pkgs[*i].name.clone()).collect()
    }

    fn query<'a>(
        filter: &'a Filter,
        tax: &'a Taxonomy,
        upgradable: &'a HashSet<String>,
        sort: SortKey,
    ) -> Query<'a> {
        Query {
            filter,
            sort,
            descending: false,
            marks: &EMPTY_MARKS,
            taxonomy: tax,
            upgradable,
        }
    }

    // MarcaSet vazio com tempo de vida estático para simplificar os testes.
    static EMPTY_MARKS: std::sync::LazyLock<MarkSet> = std::sync::LazyLock::new(MarkSet::empty);

    #[test]
    fn busca_por_prefixo_ordena_por_relevancia() {
        let pkgs = sample();
        let filter = Filter {
            text: "fire".into(),
            ..Default::default()
        };
        let out = select(&pkgs, &filter, SortKey::Name, false, &MarkSet::empty());
        let names = names(&pkgs, &out);
        assert_eq!(names[0], "firefox");
        assert!(names.contains(&"firewalld".to_string()));
        assert!(!names.contains(&"vlc".to_string()));
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

    #[test]
    fn filtra_por_categoria() {
        let pkgs = vec![
            pkg("texlive-a", 10, Reason::User, "@System"),
            pkg("vlc", 20, Reason::User, "@System"),
        ];
        let tax = Taxonomy::build(&pkgs, &CustomCategories::empty());
        let filter = Filter {
            categories: vec!["TeX Live".into()],
            ..Default::default()
        };
        let out = query(&filter, &tax, &HashSet::new(), SortKey::Name).apply(&pkgs);
        assert_eq!(names(&pkgs, &out), vec!["texlive-a".to_string()]);
    }

    #[test]
    fn filtra_por_arquitetura() {
        let mut pkgs = sample();
        pkgs[1].arch = "noarch".into();
        let filter = Filter {
            arch: Some("noarch".into()),
            ..Default::default()
        };
        let out = select(&pkgs, &filter, SortKey::Name, false, &MarkSet::empty());
        assert_eq!(names(&pkgs, &out), vec!["firewalld".to_string()]);
    }

    #[test]
    fn filtra_somente_com_atualizacao() {
        let pkgs = sample();
        let mut upgradable = HashSet::new();
        upgradable.insert("firefox".to_string());
        let filter = Filter {
            only_updates: true,
            ..Default::default()
        };
        let tax = Taxonomy::build(&pkgs, &CustomCategories::empty());
        let out = query(&filter, &tax, &upgradable, SortKey::Name).apply(&pkgs);
        assert_eq!(names(&pkgs, &out), vec!["firefox".to_string()]);
    }

    #[test]
    fn ordena_por_categoria() {
        let pkgs = vec![
            pkg("zoxide", 1, Reason::User, "@System"),
            pkg("texlive-a", 1, Reason::User, "@System"),
        ];
        let tax = Taxonomy::build(&pkgs, &CustomCategories::empty());
        let out = query(&Filter::default(), &tax, &HashSet::new(), SortKey::Category).apply(&pkgs);
        // "Outros" < "TeX Live" na ordenação lexicográfica.
        assert_eq!(
            names(&pkgs, &out),
            vec!["zoxide".to_string(), "texlive-a".to_string()]
        );
    }

    #[test]
    fn presets_de_tamanho_aplicam_limites() {
        let mut filter = Filter::default();
        filter.apply_size_preset(SizePreset::Over100MiB);
        assert_eq!(filter.min_size, Some(100 * 1024 * 1024));
        assert_eq!(filter.max_size, None);
        filter.apply_size_preset(SizePreset::Under1MiB);
        assert_eq!(filter.min_size, None);
        assert!(filter.max_size.is_some());
    }

    #[test]
    fn contagem_de_filtros_ativos() {
        let mut filter = Filter::default();
        assert_eq!(filter.active_count(), 0);
        filter.text = "vlc".into();
        filter.arch = Some("noarch".into());
        filter.categories.push("Multimídia".into());
        assert_eq!(filter.active_count(), 3);
    }
}
