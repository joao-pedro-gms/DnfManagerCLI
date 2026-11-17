//! Operações que alteram o sistema ou o estado de marcação.

use std::sync::Arc;

use dnfm_core::{format_size, DnfBackend, MarkSet, Preview};

use crate::output;

use super::{report_err, EXIT_ERROR, EXIT_OK, EXIT_PROBLEMS};

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

    // A blocklist vale para todo pedido do usuário, incluindo os de
    // --from-marks. Dependências que o solver arrasta não são conferidas aqui;
    // quem cobre isso é o próprio resolvedor, na prévia abaixo.
    for spec in &specs {
        if let Some(motivo) = dnfm_core::safety::is_protected(spec) {
            eprintln!("dnfm: {spec} é protegido ({motivo})");
            return EXIT_PROBLEMS;
        }
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
