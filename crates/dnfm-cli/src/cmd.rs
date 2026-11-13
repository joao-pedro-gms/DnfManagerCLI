//! Subcomandos do CLI. Todos compartilham o mesmo backend da TUI.

use std::sync::Arc;

use dnfm_core::{format_size, select, DnfBackend, Filter, MarkSet, Package, Preview, SortKey};

use crate::output;

/// Códigos de saída documentados no `--help`.
pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_PROBLEMS: i32 = 2;
pub const EXIT_DENIED: i32 = 3;
pub const EXIT_NO_DAEMON: i32 = 4;

pub fn exit_code(err: &dnfm_core::BackendError) -> i32 {
    match err {
        dnfm_core::BackendError::DaemonUnavailable(_) => EXIT_NO_DAEMON,
        dnfm_core::BackendError::NotAuthorized => EXIT_DENIED,
        dnfm_core::BackendError::Blocked(_) => EXIT_PROBLEMS,
        _ => EXIT_ERROR,
    }
}

pub fn report_err(err: &dnfm_core::BackendError) -> i32 {
    eprintln!("dnfm: {err}");
    exit_code(err)
}

pub struct ListOptions {
    pub pattern: Vec<String>,
    pub repo: Option<String>,
    pub sort: String,
    pub limit: Option<usize>,
    pub json: bool,
}

pub async fn list(backend: &Arc<dyn DnfBackend>, opts: ListOptions) -> i32 {
    let packages = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let marks = MarkSet::load_default();
    let mut filter = Filter {
        text: opts.pattern.join(" "),
        repo: opts.repo.clone(),
        ..Default::default()
    };
    if !opts.pattern.is_empty() {
        filter.text = opts.pattern.join(" ");
    }
    let sort = match opts.sort.as_str() {
        "size" => SortKey::Size,
        "date" => SortKey::Date,
        "repo" => SortKey::Repo,
        "reason" => SortKey::Reason,
        _ => SortKey::Name,
    };
    let selected = select(&packages, &filter, sort, false, &marks);
    let selected: Vec<&Package> = selected
        .into_iter()
        .filter_map(|i| packages.get(i))
        .take(opts.limit.unwrap_or(usize::MAX))
        .collect();

    if opts.json {
        let payload: Vec<serde_json::Value> = selected
            .iter()
            .map(|p| {
                let mut value = output::package_json(p);
                value["marked"] = serde_json::Value::Bool(marks.contains(&p.name, &p.arch));
                value
            })
            .collect();
        output::emit_json(output::envelope("list", serde_json::json!(payload)));
    } else {
        println!(
            "{} pacotes (filtro: {})",
            selected.len(),
            if filter.text.is_empty() {
                "nenhum".to_string()
            } else {
                filter.text.clone()
            }
        );
        output::table(
            &output::PACKAGE_HEADERS,
            selected
                .iter()
                .map(|p| output::package_row(p, marks.contains(&p.name, &p.arch)))
                .collect(),
        );
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

pub struct MarksOptions {
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub clear: bool,
    pub json: bool,
}

pub async fn marks(backend: &Arc<dyn DnfBackend>, opts: MarksOptions) -> i32 {
    let mut set = MarkSet::load_default();

    if !opts.add.is_empty() || !opts.remove.is_empty() || opts.clear {
        let installed = match backend.installed().await {
            Ok(list) => list,
            Err(err) => return report_err(&err),
        };
        if opts.clear {
            set.clear();
        }
        for spec in &opts.add {
            match installed.iter().find(|p| &p.name == spec) {
                Some(pkg) => {
                    if let Some(reason) = dnfm_core::safety::is_protected(&pkg.name) {
                        eprintln!("dnfm: {} é protegido ({reason})", pkg.name);
                        return EXIT_PROBLEMS;
                    }
                    set.mark(&pkg.name, &pkg.arch, &pkg.nevra, None);
                }
                None => {
                    eprintln!("dnfm: {spec} não está instalado");
                    return EXIT_ERROR;
                }
            }
        }
        for spec in &opts.remove {
            let arch = set
                .iter()
                .find(|m| &m.name == spec)
                .map(|m| m.arch.clone())
                .or_else(|| {
                    installed
                        .iter()
                        .find(|p| &p.name == spec)
                        .map(|p| p.arch.clone())
                });
            match arch {
                Some(arch) => {
                    set.unmark(spec, &arch);
                }
                None => eprintln!("dnfm: {spec} não encontrado"),
            }
        }
        if let Err(err) = set.save() {
            eprintln!("dnfm: não consegui gravar {}: {err}", set.path().display());
            return EXIT_ERROR;
        }
    }

    if opts.json {
        let payload: Vec<serde_json::Value> = set
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name, "arch": m.arch, "nevra": m.nevra, "marked_at": m.marked_at
                })
            })
            .collect();
        output::emit_json(output::envelope("marks", serde_json::json!(payload)));
    } else {
        if set.is_empty() {
            println!("nenhum pacote marcado ({})", set.path().display());
        } else {
            println!("{} marcados ({})", set.len(), set.path().display());
            for mark in set.iter() {
                println!("  {}.{}  {}", mark.name, mark.arch, mark.nevra);
            }
        }
    }
    EXIT_OK
}

pub struct RemoveOptions {
    pub specs: Vec<String>,
    pub from_marks: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
}

pub async fn remove(backend: &Arc<dyn DnfBackend>, opts: RemoveOptions) -> i32 {
    let mut specs = opts.specs.clone();
    if opts.from_marks {
        let marks = MarkSet::load_default();
        if marks.is_empty() {
            eprintln!("dnfm: nenhuma marcação registrada");
            return EXIT_ERROR;
        }
        specs.extend(marks.names());
    }
    specs.sort();
    specs.dedup();
    if specs.is_empty() {
        eprintln!("dnfm: informe pacotes ou use --from-marks");
        return EXIT_ERROR;
    }

    let preview = match backend.preview_remove(&specs).await {
        Ok(preview) => preview,
        Err(err) => return report_err(&err),
    };
    print_preview(&preview, &specs, opts.json);

    if preview.is_failed() {
        return EXIT_PROBLEMS;
    }
    if opts.dry_run {
        return EXIT_OK;
    }
    if !opts.yes {
        eprintln!("\nprévia apenas; use --yes para aplicar");
        return EXIT_OK;
    }
    let applied: Vec<(String, String)> = preview
        .requested()
        .iter()
        .map(|i| (i.package.name.clone(), i.package.arch.clone()))
        .collect();
    let token = match preview.confirm() {
        Ok(token) => token,
        Err(err) => return report_err(&err),
    };

    let mut progress = backend.progress();
    let forwarder = tokio::spawn(async move {
        while let Ok(event) = progress.recv().await {
            if let dnfm_core::ProgressEvent::ActionStart { nevra, action, .. } = event {
                eprintln!("{} {}", action.label(), nevra);
            }
        }
    });

    let outcome = backend.execute(token).await;
    forwarder.abort();
    match outcome {
        Ok(outcome) if outcome.success => {
            // Marcações aplicadas não fazem mais sentido.
            let mut marks = MarkSet::load_default();
            for (name, arch) in &applied {
                marks.unmark(name, arch);
            }
            let _ = marks.save();
            println!("transação concluída");
            EXIT_OK
        }
        Ok(_) => {
            eprintln!("dnfm: a transação terminou com falha");
            EXIT_ERROR
        }
        Err(err) => report_err(&err),
    }
}

fn print_preview(preview: &Preview, specs: &[String], json: bool) {
    if json {
        let payload = serde_json::json!({
            "result": match preview.result {
                dnfm_core::ResolveResult::Ok => "ok",
                dnfm_core::ResolveResult::Warnings => "warnings",
                dnfm_core::ResolveResult::Failed => "failed",
            },
            "freed_bytes": preview.freed_bytes(),
            "requested": preview.requested().iter().map(|i| i.package.name.clone()).collect::<Vec<_>>(),
            "collateral": preview.collateral().iter().map(|i| serde_json::json!({
                "name": i.package.name, "reason": i.reason.label(),
                "install_size": i.package.size,
            })).collect::<Vec<_>>(),
            "additions": preview.additions().map(|i| serde_json::json!({
                "name": i.package.name, "action": i.action.label(),
            })).collect::<Vec<_>>(),
            "problems": preview.problems,
            "requested_specs": specs,
        });
        output::emit_json(output::envelope("remove", payload));
        return;
    }

    if preview.is_failed() {
        println!("pedido: {}", specs.join(" "));
    }

    println!("você pediu ({})", preview.requested().len());
    for item in preview.requested() {
        println!("  − {}", item.package.name);
    }
    let collateral = preview.collateral();
    if !collateral.is_empty() {
        println!("saem junto ({})", collateral.len());
        for item in collateral {
            println!(
                "  − {} ({}, {})",
                item.package.name,
                item.reason.label(),
                format_size(item.package.size)
            );
        }
    }
    let additions: Vec<_> = preview.additions().collect();
    if !additions.is_empty() {
        println!("entram ({})", additions.len());
        for item in additions {
            println!("  + {} ({})", item.package.name, item.action.label());
        }
    }
    println!("espaço liberado: {}", format_size(preview.freed_bytes()));
    if !preview.problems.is_empty() {
        println!("\nproblemas de resolução:");
        for problem in &preview.problems {
            println!("  {problem}");
        }
    }
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
    if json {
        let payload: Vec<serde_json::Value> = list.iter().map(output::package_json).collect();
        output::emit_json(output::envelope("orphans", serde_json::json!(payload)));
        return EXIT_OK;
    }
    if list.is_empty() {
        println!("nenhum órfão encontrado");
        return EXIT_OK;
    }
    println!("{} possíveis órfãos", list.len());
    let marks = MarkSet::load_default();
    output::table(
        &output::PACKAGE_HEADERS,
        list.iter()
            .map(|p| output::package_row(p, marks.contains(&p.name, &p.arch)))
            .collect(),
    );
    EXIT_OK
}
