//! Estatísticas agregadas do sistema instalado.
//!
//! Tudo derivado do que já está em memória (pacotes, atualizações, marcações,
//! taxonomia e catálogo de metadados) — nenhuma chamada extra ao daemon.

use crate::categories::Taxonomy;
use crate::marks::MarkSet;
use crate::metadata::{tag_counts, TagCatalog};
use crate::model::{Package, Upgrade};

/// Contagem + tamanho de uma dimensão (categoria, repo, motivo…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub label: String,
    pub count: usize,
    pub size: u64,
}

impl Bucket {
    pub fn share(&self, total: usize) -> f32 {
        if total == 0 {
            0.0
        } else {
            self.count as f32 / total as f32
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub total: usize,
    pub unique_names: usize,
    pub total_size: u64,
    pub by_reason: Vec<Bucket>,
    pub by_repo: Vec<Bucket>,
    pub by_arch: Vec<Bucket>,
    pub by_category: Vec<Bucket>,
    pub top_tags: Vec<Bucket>,
    pub biggest: Vec<Bucket>,
    pub deepest: Vec<Bucket>,
    pub protected: usize,
    pub marked: usize,
    pub marked_size: u64,
    pub upgrades: usize,
    pub upgrade_bytes: u64,
    pub by_severity: Vec<Bucket>,
    pub signals: Vec<Bucket>,
    pub with_appstream: usize,
    pub with_files: usize,
}

impl Stats {
    pub fn build(
        installed: &[Package],
        upgrades: &[Upgrade],
        marks: &MarkSet,
        taxonomy: &Taxonomy,
        catalog: &TagCatalog,
    ) -> Self {
        let mut stats = Stats {
            total: installed.len(),
            ..Default::default()
        };
        if installed.is_empty() {
            stats.upgrades = upgrades.len();
            return stats;
        }

        let mut nomes = std::collections::HashSet::new();
        let mut por_motivo: std::collections::BTreeMap<String, (usize, u64)> = Default::default();
        let mut por_repo: std::collections::BTreeMap<String, (usize, u64)> = Default::default();
        let mut por_arch: std::collections::BTreeMap<String, (usize, u64)> = Default::default();
        let mut por_categoria: std::collections::BTreeMap<String, (usize, u64)> =
            Default::default();
        let mut sinais: std::collections::BTreeMap<String, usize> = Default::default();

        for pkg in installed {
            stats.total_size += pkg.size;
            nomes.insert(pkg.name.clone());

            let motivo = pkg.reason.label().to_string();
            let repo = pkg.repo_id.clone();
            let arch = pkg.arch.clone();
            let categoria = taxonomy.category_of(pkg).to_string();

            let e = por_motivo.entry(motivo).or_insert((0, 0));
            e.0 += 1;
            e.1 += pkg.size;
            let e = por_repo.entry(repo).or_insert((0, 0));
            e.0 += 1;
            e.1 += pkg.size;
            let e = por_arch.entry(arch).or_insert((0, 0));
            e.0 += 1;
            e.1 += pkg.size;
            let e = por_categoria.entry(categoria).or_insert((0, 0));
            e.0 += 1;
            e.1 += pkg.size;

            if let Some(files) = catalog.files.get(&pkg.name) {
                for label in files.signals.labels() {
                    *sinais.entry(label.to_string()).or_insert(0) += 1;
                }
            }

            if crate::safety::is_protected(&pkg.name).is_some() {
                stats.protected += 1;
            }
            if marks.contains(&pkg.name, &pkg.arch) {
                stats.marked += 1;
                stats.marked_size += pkg.size;
            }
        }

        stats.unique_names = nomes.len();
        stats.with_appstream = installed
            .iter()
            .filter(|p| catalog.appstream.contains_key(&p.name))
            .count();
        stats.with_files = installed
            .iter()
            .filter(|p| catalog.files.contains_key(&p.name))
            .count();

        stats.by_reason = buckets(por_motivo);
        stats.by_repo = buckets(por_repo);
        stats.by_arch = buckets(por_arch);
        stats.by_category = buckets(por_categoria);

        stats.top_tags = tag_counts(installed, catalog)
            .into_iter()
            .map(|(label, count)| Bucket {
                label,
                count,
                size: 0,
            })
            .collect();
        stats
            .top_tags
            .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
        stats.top_tags.truncate(20);

        // Várias versões do mesmo pacote (installonly, como kernels) colapsam
        // numa linha só, senão a lista parece ter duplicatas.
        stats.biggest = colapsar(
            installed.iter().map(|p| (p.name.clone(), p.size, 1usize)),
            10,
            false,
        );
        stats.deepest = colapsar(
            installed.iter().filter_map(|p| {
                catalog
                    .files
                    .get(&p.name)
                    .map(|f| (p.name.clone(), p.size, f.count))
            }),
            10,
            true,
        );

        stats.signals = sinais
            .into_iter()
            .map(|(label, count)| Bucket {
                label,
                count,
                size: 0,
            })
            .collect();
        stats.signals.sort_by(|a, b| b.count.cmp(&a.count));

        stats.upgrades = upgrades.len();
        stats.upgrade_bytes = upgrades.iter().map(|u| u.candidate.size).sum();
        let mut severidade: std::collections::BTreeMap<String, usize> = Default::default();
        for up in upgrades {
            *severidade
                .entry(up.severity.label().to_string())
                .or_insert(0) += 1;
        }
        stats.by_severity = severidade
            .into_iter()
            .map(|(label, count)| Bucket {
                label,
                count,
                size: 0,
            })
            .collect();
        stats.by_severity.sort_by(|a, b| b.count.cmp(&a.count));

        stats
    }
}

/// Agrupa por nome, somando versões diferentes do mesmo pacote (installonly).
fn colapsar(
    itens: impl IntoIterator<Item = (String, u64, usize)>,
    limite: usize,
    por_peso: bool,
) -> Vec<Bucket> {
    let mut mapa: std::collections::BTreeMap<String, (u64, usize, usize)> = Default::default();
    for (nome, size, peso) in itens {
        let entrada = mapa.entry(nome).or_insert((0, 0, 0));
        entrada.0 = entrada.0.max(size);
        entrada.1 += peso;
        entrada.2 += 1;
    }
    let mut lista: Vec<Bucket> = mapa
        .into_iter()
        .map(|(nome, (size, peso, versoes))| Bucket {
            label: if versoes > 1 {
                format!("{nome} ×{versoes}")
            } else {
                nome
            },
            count: peso,
            size,
        })
        .collect();
    if por_peso {
        lista.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| b.size.cmp(&a.size)));
    } else {
        lista.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| b.count.cmp(&a.count)));
    }
    lista.truncate(limite);
    lista
}

fn buckets(mapa: std::collections::BTreeMap<String, (usize, u64)>) -> Vec<Bucket> {
    let mut lista: Vec<Bucket> = mapa
        .into_iter()
        .map(|(label, (count, size))| Bucket { label, count, size })
        .collect();
    lista.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    lista
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categories::CustomCategories;
    use crate::metadata::{FileSignals, PackageFiles, TagCatalog};
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
            summary: String::new(),
            reason,
            nevra: format!("{name}-1-1.fc45.x86_64"),
            is_installed: true,
        }
    }

    #[test]
    fn agrega_totais_e_dimensoes() {
        let pacotes = vec![
            pkg("vlc", 100, Reason::User, "@System"),
            pkg("texlive-a", 50, Reason::Dependency, "@System"),
            pkg("texlive-b", 50, Reason::Dependency, "@System"),
        ];
        let tax = Taxonomy::build(&pacotes, &CustomCategories::empty());
        let stats = Stats::build(
            &pacotes,
            &[],
            &MarkSet::empty(),
            &tax,
            &TagCatalog::default(),
        );

        assert_eq!(stats.total, 3);
        assert_eq!(stats.unique_names, 3);
        assert_eq!(stats.total_size, 200);
        assert_eq!(stats.by_reason[0].label, "dependência");
        assert_eq!(stats.by_reason[0].count, 2);
        assert_eq!(stats.by_category[0].label, "TeX Live");
        assert_eq!(stats.by_category[0].count, 2);
        assert_eq!(stats.biggest[0].label, "vlc");
    }

    #[test]
    fn conta_marcados_e_protegidos() {
        let pacotes = vec![
            pkg("vlc", 100, Reason::User, "@System"),
            pkg("glibc", 900, Reason::Dependency, "@System"),
        ];
        let mut marks = MarkSet::empty();
        marks.mark("vlc", "x86_64", "vlc-1-1.fc45.x86_64", None);
        let tax = Taxonomy::build(&pacotes, &CustomCategories::empty());
        let stats = Stats::build(&pacotes, &[], &marks, &tax, &TagCatalog::default());
        assert_eq!(stats.marked, 1);
        assert_eq!(stats.marked_size, 100);
        assert_eq!(stats.protected, 1);
    }

    #[test]
    fn usa_sinais_de_arquivo_quando_existem() {
        let pacotes = vec![pkg("ripgrep", 10, Reason::User, "@System")];
        let mut catalog = TagCatalog::default();
        let mut files = PackageFiles {
            count: 4,
            ..Default::default()
        };
        files.signals.set(FileSignals::BIN);
        files.signals.set(FileSignals::MAN);
        catalog.files.insert("ripgrep".into(), files);

        let tax = Taxonomy::build(&pacotes, &CustomCategories::empty());
        let stats = Stats::build(&pacotes, &[], &MarkSet::empty(), &tax, &catalog);
        assert_eq!(stats.with_files, 1);
        assert!(stats
            .signals
            .iter()
            .any(|s| s.label == "comando" && s.count == 1));
        assert!(stats.top_tags.iter().any(|t| t.label == "man"));
        assert_eq!(stats.deepest[0].count, 4);
    }

    #[test]
    fn soma_o_tamanho_das_atualizacoes() {
        let pacotes = vec![pkg("vlc", 100, Reason::User, "@System")];
        let upgrades = vec![Upgrade {
            installed: pacotes[0].clone(),
            candidate: Package {
                size: 120,
                ..pacotes[0].clone()
            },
            severity: crate::model::Severity::Critical,
            advisory: None,
        }];
        let tax = Taxonomy::build(&pacotes, &CustomCategories::empty());
        let stats = Stats::build(
            &pacotes,
            &upgrades,
            &MarkSet::empty(),
            &tax,
            &TagCatalog::default(),
        );
        assert_eq!(stats.upgrades, 1);
        assert_eq!(stats.upgrade_bytes, 120);
        assert_eq!(stats.by_severity[0].label, "crítica");
    }

    #[test]
    fn sistema_vazio_nao_divide_por_zero() {
        let stats = Stats::default();
        assert_eq!(stats.by_reason.len(), 0);
        assert_eq!(
            Bucket {
                label: "x".into(),
                count: 0,
                size: 0
            }
            .share(0),
            0.0
        );
    }
}
