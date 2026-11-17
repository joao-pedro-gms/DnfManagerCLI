//! Listagens: pacotes, atualizações, histórico, repos e órfãos.

use std::collections::HashSet;
use std::sync::Arc;

use dnfm_core::categories::{group_rows, CustomCategories, GroupMode, Taxonomy};
use dnfm_core::safety;
use dnfm_core::{DnfBackend, Filter, MarkSet, Query, SortKey};

use crate::output;

use super::{report_err, EXIT_OK};

pub struct ListOptions {
    pub pattern: Vec<String>,
    pub repo: Option<String>,
    pub category: Vec<String>,
    pub family: Vec<String>,
    pub arch: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub installed_since_days: Option<i64>,
    pub with_updates: bool,
    pub sort: String,
    pub group_by: String,
    pub limit: Option<usize>,
    pub json: bool,
}

pub async fn list(backend: &Arc<dyn DnfBackend>, opts: ListOptions) -> i32 {
    let packages = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let marks = MarkSet::load_default();
    let custom = CustomCategories::load_default();
    // O catálogo automático só é necessário para filtrar por categoria/família
    // (custa ~1,6 s mesmo com cache), então listas simples seguem rápidas.
    let precisa_catalogo = !opts.category.is_empty() || !opts.family.is_empty();
    let catalog = if precisa_catalogo {
        super::load_catalog(backend, &packages).await.ok()
    } else {
        None
    };
    let taxonomy = match catalog.as_ref() {
        Some(catalog) => Taxonomy::build_with(&packages, &custom, Some(catalog)),
        None => Taxonomy::build(&packages, &custom),
    };

    let mut upgradable = HashSet::new();
    if opts.with_updates {
        match backend.upgrades().await {
            Ok(list) => {
                for up in list {
                    upgradable.insert(up.installed.name);
                }
            }
            Err(err) => return report_err(&err),
        }
    }

    let mut filter = Filter {
        repo: opts.repo.clone(),
        arch: opts.arch.clone(),
        categories: opts.category.clone(),
        families: opts.family.clone(),
        min_size: opts.min_size,
        max_size: opts.max_size,
        only_updates: opts.with_updates,
        ..Default::default()
    };
    if !opts.pattern.is_empty() {
        filter.text = opts.pattern.join(" ");
    }
    if let Some(days) = opts.installed_since_days {
        filter.installed_since = Some(dnfm_core::marks::now_unix() - days.max(0) * 86_400);
    }

    let sort = SortKey::parse(&opts.sort).unwrap_or(SortKey::Name);
    let group_by = GroupMode::parse(&opts.group_by).unwrap_or(GroupMode::None);

    let visible = Query {
        filter: &filter,
        sort,
        descending: false,
        marks: &marks,
        taxonomy: &taxonomy,
        upgradable: &upgradable,
    }
    .apply(&packages);
    let limit = opts.limit.unwrap_or(usize::MAX);
    let total = visible.len();
    let visible: Vec<usize> = visible.into_iter().take(limit).collect();
    let contagem = if visible.len() == total {
        format!("{total} pacotes")
    } else {
        format!("{} de {} pacotes", visible.len(), total)
    };

    if opts.json {
        let items: Vec<serde_json::Value> = visible
            .iter()
            .map(|idx| {
                let pkg = &packages[*idx];
                let mut value = output::package_json(pkg);
                value["category"] = serde_json::json!(taxonomy.category_of(pkg));
                value["family"] = serde_json::json!(taxonomy.family_of(pkg));
                value["marked"] = serde_json::json!(marks.contains(&pkg.name, &pkg.arch));
                value
            })
            .collect();
        let mut payload = serde_json::json!({
            "total": total,
            "returned": visible.len(),
            "packages": items,
        });
        if group_by != GroupMode::None {
            let groups: Vec<serde_json::Value> =
                group_rows(&packages, &visible, group_by, &taxonomy)
                    .into_iter()
                    .map(|group| {
                        serde_json::json!({
                            "label": group.label,
                            "count": group.len(),
                            "packages": group.members.iter()
                                .map(|i| packages[*i].name.clone())
                                .collect::<Vec<_>>(),
                        })
                    })
                    .collect();
            payload["group_by"] = serde_json::json!(group_by.key());
            payload["groups"] = serde_json::json!(groups);
        }
        output::emit_json(output::envelope("list", payload));
        return EXIT_OK;
    }

    if group_by == GroupMode::None {
        println!("{contagem}");
        output::table(
            &output::PACKAGE_HEADERS,
            visible
                .iter()
                .map(|idx| {
                    let pkg = &packages[*idx];
                    output::package_row(
                        pkg,
                        marks.contains(&pkg.name, &pkg.arch),
                        taxonomy.category_of(pkg),
                    )
                })
                .collect(),
        );
    } else {
        println!(
            "{contagem} em {} grupo(s), por {}",
            group_rows(&packages, &visible, group_by, &taxonomy).len(),
            group_by.label()
        );
        for group in group_rows(&packages, &visible, group_by, &taxonomy) {
            println!("\n── {} ({})", group.label, group.len());
            output::table(
                &output::PACKAGE_HEADERS,
                group
                    .members
                    .iter()
                    .map(|idx| {
                        let pkg = &packages[*idx];
                        output::package_row(
                            pkg,
                            marks.contains(&pkg.name, &pkg.arch),
                            taxonomy.category_of(pkg),
                        )
                    })
                    .collect(),
            );
        }
    }
    EXIT_OK
}

pub struct UpdatesOptions {
    pub security_only: bool,
    pub json: bool,
}

pub async fn updates(backend: &Arc<dyn DnfBackend>, opts: UpdatesOptions) -> i32 {
    let upgrades = match backend.upgrades().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let filtered: Vec<_> = upgrades
        .iter()
        .filter(|u| !opts.security_only || u.severity.is_security_relevant())
        .collect();

    if opts.json {
        let payload: Vec<serde_json::Value> =
            filtered.iter().map(|u| output::upgrade_json(u)).collect();
        output::emit_json(output::envelope("updates", serde_json::json!(payload)));
    } else if filtered.is_empty() {
        println!("nada para atualizar");
    } else {
        println!("{} atualizações disponíveis", filtered.len());
        let rows = filtered
            .iter()
            .map(|u| {
                vec![
                    u.installed.name.clone(),
                    u.installed.evr.clone(),
                    u.candidate.evr.clone(),
                    u.severity.label().to_string(),
                    u.advisory.clone().unwrap_or_else(|| "—".into()),
                ]
            })
            .collect();
        output::table(
            &["nome", "instalado", "disponível", "severidade", "advisory"],
            rows,
        );
    }
    EXIT_OK
}

pub struct HistoryOptions {
    pub limit: i64,
    pub json: bool,
}

pub async fn history(backend: &Arc<dyn DnfBackend>, opts: HistoryOptions) -> i32 {
    let entries = match backend.history(opts.limit).await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    if opts.json {
        let payload: Vec<serde_json::Value> = entries.iter().map(output::history_json).collect();
        output::emit_json(output::envelope("history", serde_json::json!(payload)));
        return EXIT_OK;
    }
    let rows = entries
        .iter()
        .map(|e| {
            vec![
                e.id.to_string(),
                dnfm_core::format_date(e.start),
                e.status.clone(),
                e.changes().to_string(),
                e.description.clone(),
            ]
        })
        .collect();
    output::table(&["id", "data", "status", "mudanças", "comando"], rows);
    EXIT_OK
}

pub async fn repos(backend: &Arc<dyn DnfBackend>, json: bool) -> i32 {
    let repos = match backend.repos().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    if json {
        let payload: Vec<serde_json::Value> = repos.iter().map(output::repo_json).collect();
        output::emit_json(output::envelope("repos", serde_json::json!(payload)));
        return EXIT_OK;
    }
    let rows = repos
        .iter()
        .map(|r| {
            vec![
                r.id.clone(),
                r.name.clone(),
                if r.enabled { "ativo" } else { "inativo" }.to_string(),
            ]
        })
        .collect();
    output::table(&["id", "nome", "estado"], rows);
    EXIT_OK
}

pub async fn orphans(backend: &Arc<dyn DnfBackend>, json: bool) -> i32 {
    let list = match backend.orphans().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    // Pacote protegido nunca é sugestão de remoção: sai da lista, mas a
    // contagem fica à vista para a heurística não esconder o que apontou.
    let (protegidos, list): (Vec<_>, Vec<_>) = list
        .into_iter()
        .partition(|p| safety::is_protected(&p.name).is_some());
    let protected_skipped = protegidos.len();

    if json {
        let payload = serde_json::json!({
            "count": list.len(),
            "protected_skipped": protected_skipped,
            "packages": list.iter().map(output::package_json).collect::<Vec<_>>(),
        });
        output::emit_json(output::envelope("orphans", payload));
        return EXIT_OK;
    }
    let sufixo = if protected_skipped > 0 {
        format!(" ({protected_skipped} protegidos ignorados)")
    } else {
        String::new()
    };
    if list.is_empty() {
        println!("nenhum órfão encontrado{sufixo}");
        return EXIT_OK;
    }
    println!("{} possíveis órfãos{sufixo}", list.len());
    let marks = MarkSet::load_default();
    let taxonomy = Taxonomy::build(&list, &CustomCategories::load_default());
    output::table(
        &output::PACKAGE_HEADERS,
        list.iter()
            .map(|p| {
                output::package_row(p, marks.contains(&p.name, &p.arch), taxonomy.category_of(p))
            })
            .collect(),
    );
    EXIT_OK
}
