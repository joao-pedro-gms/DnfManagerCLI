//! Filtro, busca e ordenação sobre a lista de pacotes em memória.
//!
//! A lista inteira (8k pacotes) cabe folgada na memória; filtrar é mais rápido
//! do que voltar ao daemon a cada tecla.

mod filter;
mod presets;

pub use filter::{score_package, Filter, SortKey};
pub use presets::{DatePreset, SizePreset};

use std::collections::HashSet;

use crate::categories::Taxonomy;
use crate::marks::MarkSet;
use crate::model::Package;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categories::CustomCategories;
    use crate::model::{year_start, Reason};

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
    fn preset_este_ano_comeca_em_primeiro_de_janeiro() {
        assert_eq!(
            DatePreset::ThisYear.since(1_781_481_600),
            Some(1_767_225_600)
        );
        assert_eq!(
            DatePreset::ThisYear.since(1_718_409_600),
            Some(1_704_067_200)
        );
        assert_eq!(
            crate::model::format_date(year_start(1_781_481_600)),
            "2026-01-01"
        );
        assert_eq!(
            crate::model::format_date(year_start(1_718_409_600)),
            "2024-01-01"
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
