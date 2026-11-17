//! Categorias próprias: listar, atribuir e remover.

use std::sync::Arc;

use dnfm_core::categories::{CustomCategories, Taxonomy};
use dnfm_core::DnfBackend;

use crate::output;

use super::{report_err, EXIT_ERROR, EXIT_OK};

pub enum CategoryAction {
    List,
    Assign {
        label: String,
        packages: Vec<String>,
    },
    Remove {
        packages: Vec<String>,
    },
    Clear,
}

pub struct CategoriesOptions {
    pub action: CategoryAction,
    pub json: bool,
}

/// Lista e edita as categorias próprias do usuário.
pub async fn categories(backend: &Arc<dyn DnfBackend>, opts: CategoriesOptions) -> i32 {
    let installed = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let mut custom = CustomCategories::load_default();

    match opts.action {
        CategoryAction::List => {
            let taxonomy = match super::load_catalog(backend, &installed).await.ok() {
                Some(catalog) => Taxonomy::build_with(&installed, &custom, Some(&catalog)),
                None => Taxonomy::build(&installed, &custom),
            };
            let mut items: Vec<&dnfm_core::categories::CategoryInfo> =
                taxonomy.categories().iter().collect();
            items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
            if opts.json {
                let payload: Vec<serde_json::Value> = items
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "label": c.label,
                            "count": c.count,
                            "custom": c.custom,
                        })
                    })
                    .collect();
                output::emit_json(output::envelope("categories", serde_json::json!(payload)));
            } else {
                println!("{} categorias", items.len());
                output::table(
                    &["categoria", "pacotes", "origem"],
                    items
                        .iter()
                        .map(|c| {
                            vec![
                                c.label.clone(),
                                c.count.to_string(),
                                if c.custom { "própria" } else { "curada" }.to_string(),
                            ]
                        })
                        .collect(),
                );
            }
            EXIT_OK
        }
        CategoryAction::Assign { label, packages } => {
            if packages.is_empty() {
                eprintln!("dnfm: informe ao menos um pacote");
                return EXIT_ERROR;
            }
            let mut targets = Vec::new();
            for spec in &packages {
                match installed.iter().find(|p| &p.name == spec) {
                    Some(pkg) => targets.push(pkg.clone()),
                    None => {
                        eprintln!("dnfm: {spec} não está instalado");
                        return EXIT_ERROR;
                    }
                }
            }
            let count = targets.len();
            custom.assign(&targets, &label);
            if let Err(err) = custom.save() {
                eprintln!(
                    "dnfm: não consegui gravar {}: {err}",
                    custom.path().display()
                );
                return EXIT_ERROR;
            }
            if opts.json {
                output::emit_json(output::envelope(
                    "categories",
                    serde_json::json!({ "assigned": count, "label": label }),
                ));
            } else {
                println!("{count} pacote(s) em «{label}»");
            }
            EXIT_OK
        }
        CategoryAction::Remove { packages } => {
            if packages.is_empty() {
                return EXIT_ERROR;
            }
            let mut removed = 0;
            for spec in &packages {
                let arch = installed
                    .iter()
                    .find(|p| &p.name == spec)
                    .map(|p| p.arch.clone())
                    .or_else(|| {
                        custom
                            .iter()
                            .find(|a| &a.name == spec)
                            .map(|a| a.arch.clone())
                    });
                if let Some(arch) = arch {
                    if custom.unassign(spec, &arch) {
                        removed += 1;
                        continue;
                    }
                }
                eprintln!("dnfm: {spec} não tem categoria própria");
            }
            if let Err(err) = custom.save() {
                eprintln!("dnfm: não consegui gravar: {err}");
                return EXIT_ERROR;
            }
            if opts.json {
                output::emit_json(output::envelope(
                    "categories",
                    serde_json::json!({ "removed": removed }),
                ));
            } else {
                println!("{removed} categoria(s) removida(s)");
            }
            EXIT_OK
        }
        CategoryAction::Clear => {
            let count = custom.len();
            custom.clear();
            if let Err(err) = custom.save() {
                eprintln!("dnfm: não consegui gravar: {err}");
                return EXIT_ERROR;
            }
            if opts.json {
                output::emit_json(output::envelope(
                    "categories",
                    serde_json::json!({ "cleared": count }),
                ));
            } else {
                println!("{count} atribuição(ões) removida(s)");
            }
            EXIT_OK
        }
    }
}
