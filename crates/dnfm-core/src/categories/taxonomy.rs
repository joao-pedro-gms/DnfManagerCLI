use std::collections::{BTreeMap, HashMap};

use super::rules::{alias_for, prefix, FALLBACK_CATEGORY};
use super::{categorize, family, package_key, CustomCategories, GroupMode};
use crate::metadata::TagCatalog;
use crate::model::Package;

const OTHER_FAMILY: &str = "Outros";
/// Famílias menores que isso caem em "Outros" no agrupamento por família.
const MIN_FAMILY_SIZE: usize = 3;

/// Uma categoria visível na interface, com contagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryInfo {
    pub label: String,
    pub count: usize,
    pub custom: bool,
}

/// Índice de categorias e famílias para um conjunto de pacotes.
#[derive(Debug, Clone, Default)]
pub struct Taxonomy {
    categories: Vec<CategoryInfo>,
    category_of: HashMap<String, String>,
    family_of: HashMap<String, String>,
}

impl Taxonomy {
    pub fn build(packages: &[Package], custom: &CustomCategories) -> Self {
        Self::build_with(packages, custom, None)
    }

    /// Como [`Self::build`], mas usando o catálogo automático (AppStream,
    /// .desktop e domínio da URL) antes de cair nas regras curadas.
    pub fn build_with(
        packages: &[Package],
        custom: &CustomCategories,
        catalog: Option<&TagCatalog>,
    ) -> Self {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut custom_labels: BTreeMap<String, usize> = BTreeMap::new();
        let mut category_of = HashMap::with_capacity(packages.len());
        let mut family_of = HashMap::with_capacity(packages.len());

        let mut family_counts: BTreeMap<String, usize> = BTreeMap::new();
        for pkg in packages {
            family_counts
                .entry(family(pkg))
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        for pkg in packages {
            let key = package_key(&pkg.name, &pkg.arch);
            let label = match custom.get(&pkg.name, &pkg.arch) {
                Some(label) => {
                    custom_labels
                        .entry(label.to_string())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    label.to_string()
                }
                None => catalog
                    .and_then(|c| c.category_for(pkg))
                    .map(|tag| tag.label)
                    .unwrap_or_else(|| categorize(pkg)),
            };
            counts
                .entry(label.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            category_of.insert(key.clone(), label);

            let name = pkg.name.to_ascii_lowercase();
            // 1) domínio da URL, quando identifica o projeto; 2) apelido do
            // prefixo; 3) prefixo com limiar de tamanho.
            let fam = match catalog.and_then(|c| c.family_for(&pkg.name)) {
                Some(label) => label.to_string(),
                None => match alias_for(prefix(&name)) {
                    Some(label) => label.to_string(),
                    None => {
                        let raw = family(pkg);
                        if family_counts.get(&raw).copied().unwrap_or(0) < MIN_FAMILY_SIZE {
                            OTHER_FAMILY.to_string()
                        } else {
                            raw
                        }
                    }
                },
            };
            family_of.insert(key, fam);
        }

        let categories = counts
            .into_iter()
            .map(|(label, count)| CategoryInfo {
                custom: custom_labels.contains_key(&label),
                label,
                count,
            })
            .collect();

        Self {
            categories,
            category_of,
            family_of,
        }
    }

    pub fn categories(&self) -> &[CategoryInfo] {
        &self.categories
    }

    pub fn category_of(&self, pkg: &Package) -> &str {
        self.category_of
            .get(&package_key(&pkg.name, &pkg.arch))
            .map(String::as_str)
            .unwrap_or(FALLBACK_CATEGORY)
    }

    pub fn family_of(&self, pkg: &Package) -> &str {
        self.family_of
            .get(&package_key(&pkg.name, &pkg.arch))
            .map(String::as_str)
            .unwrap_or(OTHER_FAMILY)
    }

    /// Rótulo usado pelo modo de agrupamento pedido.
    pub fn label_for(&self, pkg: &Package, mode: GroupMode) -> String {
        match mode {
            GroupMode::None => String::new(),
            GroupMode::Category => self.category_of(pkg).to_string(),
            GroupMode::Family => self.family_of(pkg).to_string(),
            GroupMode::Repo => pkg.repo_id.clone(),
            GroupMode::Reason => pkg.reason.label().to_string(),
        }
    }

    pub fn category_count(&self) -> usize {
        self.categories.len()
    }
}

/// Um grupo pronto para renderização: rótulo + índices na lista de pacotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: String,
    pub members: Vec<usize>,
}

impl Group {
    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Agrupa a seleção preservando a ordem recebida; grupos ordenados por tamanho.
pub fn group_rows(
    packages: &[Package],
    visible: &[usize],
    mode: GroupMode,
    taxonomy: &Taxonomy,
) -> Vec<Group> {
    if mode == GroupMode::None {
        return vec![Group {
            label: String::new(),
            members: visible.to_vec(),
        }];
    }

    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for idx in visible {
        let Some(pkg) = packages.get(*idx) else {
            continue;
        };
        buckets
            .entry(taxonomy.label_for(pkg, mode))
            .or_default()
            .push(*idx);
    }

    let mut groups: Vec<Group> = buckets
        .into_iter()
        .map(|(label, members)| Group { label, members })
        .collect();
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.label.cmp(&b.label)));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reason;
    use std::fs;

    fn pkg(name: &str) -> Package {
        Package {
            name: name.into(),
            evr: "1-1.fc45".into(),
            arch: "x86_64".into(),
            repo_id: "@System".into(),
            from_repo_id: None,
            size: 1024,
            install_time: 0,
            summary: String::new(),
            reason: Reason::User,
            nevra: format!("{name}-1-1.fc45.x86_64"),
            is_installed: true,
        }
    }

    #[test]
    fn prefixo_conhecido_vira_categoria_curada() {
        assert_eq!(categorize(&pkg("texlive-a2ping")), "TeX Live");
        assert_eq!(categorize(&pkg("kf6-kcoreaddons")), "KDE Plasma");
        assert_eq!(categorize(&pkg("python3-requests")), "Python");
        assert_eq!(categorize(&pkg("perl-Getopt-Long")), "Perl");
    }

    #[test]
    fn palavra_chave_categoriza_o_que_o_prefixo_nao_pega() {
        assert_eq!(categorize(&pkg("amd-gpu-firmware")), "Firmware");
        assert_eq!(categorize(&pkg("default-fonts-core-sans")), "Fontes");
        assert_eq!(categorize(&pkg("gtk3-devel")), "GNOME");
        assert_eq!(categorize(&pkg("openssl-devel")), "Segurança");
        assert_eq!(categorize(&pkg("libxml2")), "Bibliotecas");
    }

    #[test]
    fn prefixo_desconhecido_cai_em_outros() {
        assert_eq!(categorize(&pkg("zoxide")), "Outros");
    }

    #[test]
    fn familia_agrupa_parentes() {
        assert_eq!(family(&pkg("kf5-kio")), "KDE Plasma");
        assert_eq!(family(&pkg("kf6-kio")), "KDE Plasma");
        assert_eq!(family(&pkg("zoxide")), "Zoxide");
    }

    #[test]
    fn categoria_customizada_vence_a_curada() {
        let packages = vec![pkg("vlc")];
        let mut custom = CustomCategories::empty();
        custom.assign(&packages, "Multimídia pessoal");
        let tax = Taxonomy::build(&packages, &custom);
        assert_eq!(tax.category_of(&packages[0]), "Multimídia pessoal");
        assert!(tax.categories().iter().any(|c| c.custom && c.count == 1));
    }

    #[test]
    fn familias_pequenas_vao_para_outros() {
        let packages = vec![pkg("zoxide"), pkg("bat"), pkg("vlc"), pkg("vlc-plugin")];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        assert_eq!(tax.family_of(&packages[0]), "Outros");
        assert_eq!(tax.family_of(&packages[2]), "Multimídia");
    }

    #[test]
    fn agrupa_por_categoria_ordenando_por_tamanho() {
        let packages = vec![
            pkg("texlive-a"),
            pkg("texlive-b"),
            pkg("vlc"),
            pkg("zoxide"),
        ];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        let visible: Vec<usize> = (0..packages.len()).collect();
        let groups = group_rows(&packages, &visible, GroupMode::Category, &tax);
        assert_eq!(groups[0].label, "TeX Live");
        assert_eq!(groups[0].len(), 2);
        assert!(groups.iter().any(|g| g.label == "Outros"));
    }

    #[test]
    fn sem_grupo_devolve_um_unico_bloco() {
        let packages = vec![pkg("vlc"), pkg("zoxide")];
        let tax = Taxonomy::build(&packages, &CustomCategories::empty());
        let visible = vec![0, 1];
        let groups = group_rows(&packages, &visible, GroupMode::None, &tax);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members, visible);
    }

    #[test]
    fn persistencia_de_categorias() {
        let dir = std::env::temp_dir().join(format!("dnfm-cat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("categories.json");
        let packages = vec![pkg("vlc")];
        let mut custom = CustomCategories::load(&path);
        custom.assign(&packages, "Multimídia");
        custom.save().expect("gravar");

        let reloaded = CustomCategories::load(&path);
        assert_eq!(reloaded.get("vlc", "x86_64"), Some("Multimídia"));
        assert_eq!(reloaded.labels(), vec!["Multimídia".to_string()]);
    }

    #[test]
    fn prune_descarta_pacotes_ausentes() {
        let dir = std::env::temp_dir().join(format!("dnfm-cat2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut custom = CustomCategories::load(dir.join("categories.json"));
        custom.assign(&[pkg("vlc")], "Multimídia");
        custom.assign(&[pkg("firefox")], "Internet");
        assert_eq!(custom.prune(&["vlc"]), 1);
        assert!(custom.get("vlc", "x86_64").is_some());
        assert!(custom.get("firefox", "x86_64").is_none());
    }
    #[test]
    fn appstream_tem_prioridade_sobre_a_regra_curada() {
        let pacotes = vec![pkg("texlive-foo")];
        let mut catalog = crate::metadata::TagCatalog::default();
        catalog.appstream.insert(
            "texlive-foo".into(),
            crate::metadata::AppStreamComponent {
                id: "org.exemplo.Foo.desktop".into(),
                categories: vec!["Graphics".into()],
                summary: None,
            },
        );
        let sem_catalogo = Taxonomy::build(&pacotes, &CustomCategories::empty());
        assert_eq!(sem_catalogo.category_of(&pacotes[0]), "TeX Live");

        let com_catalogo =
            Taxonomy::build_with(&pacotes, &CustomCategories::empty(), Some(&catalog));
        assert_eq!(com_catalogo.category_of(&pacotes[0]), "Gráficos");
    }

    #[test]
    fn familia_vem_do_dominio_da_url() {
        let pacotes = vec![pkg("pacote-sem-prefixo-conhecido")];
        let mut catalog = crate::metadata::TagCatalog::default();
        catalog
            .families
            .insert("pacote-sem-prefixo-conhecido".into(), "Zig".into());
        let tax = Taxonomy::build_with(&pacotes, &CustomCategories::empty(), Some(&catalog));
        assert_eq!(tax.family_of(&pacotes[0]), "Zig");
    }
}
