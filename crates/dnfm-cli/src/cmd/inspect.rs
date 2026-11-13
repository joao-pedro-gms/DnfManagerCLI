//! Inspeção de pacotes, estatísticas e tags automáticas.

use std::sync::Arc;

use dnfm_core::categories::{CustomCategories, Taxonomy};
use dnfm_core::{DnfBackend, MarkSet, Stats, TagCatalog};

use crate::output;

use super::{report_err, EXIT_ERROR, EXIT_OK};

pub struct InspectOptions {
    pub target: String,
    pub json: bool,
}

pub struct StatsOptions {
    pub json: bool,
}

pub struct TagsOptions {
    pub json: bool,
    pub limit: usize,
}

pub async fn stats(backend: &Arc<dyn DnfBackend>, opts: StatsOptions) -> i32 {
    let packages = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let upgrades = match backend.upgrades().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let marks = MarkSet::load_default();
    let custom = CustomCategories::load_default();
    let catalog = match super::load_catalog(backend, &packages).await {
        Ok(catalog) => catalog,
        Err(err) => {
            eprintln!("dnfm: aviso: seguindo sem metadados automáticos ({err})");
            TagCatalog::default()
        }
    };
    let taxonomy = Taxonomy::build_with(&packages, &custom, Some(&catalog));
    let stats = Stats::build(&packages, &upgrades, &marks, &taxonomy, &catalog);

    if opts.json {
        output::emit_json(output::envelope("stats", stats_json(&stats)));
        return EXIT_OK;
    }

    println!(
        "{} pacotes instalados · {} nomes · {} em disco",
        stats.total,
        stats.unique_names,
        dnfm_core::format_size(stats.total_size)
    );
    println!(
        "{} protegidos · {} marcados ({}) · {} atualizações ({})",
        stats.protected,
        stats.marked,
        dnfm_core::format_size(stats.marked_size),
        stats.upgrades,
        dnfm_core::format_size(stats.upgrade_bytes)
    );
    println!(
        "cobertura: {} com AppStream ({:.1}%) · {} com lista de arquivos ({:.1}%)",
        stats.with_appstream,
        percent(stats.with_appstream, stats.total),
        stats.with_files,
        percent(stats.with_files, stats.total)
    );

    secao("POR CATEGORIA", &stats.by_category, stats.total, true);
    secao("POR MOTIVO", &stats.by_reason, stats.total, true);
    secao("POR REPOSITÓRIO", &stats.by_repo, stats.total, true);
    secao("TIPO DOS PACOTES", &stats.signals, stats.total, false);
    secao("MAIORES", &stats.biggest, 0, true);
    secao("COM MAIS ARQUIVOS", &stats.deepest, 0, false);
    secao(
        "ATUALIZAÇÕES POR SEVERIDADE",
        &stats.by_severity,
        stats.upgrades,
        false,
    );

    EXIT_OK
}

fn secao(titulo: &str, buckets: &[dnfm_core::Bucket], total: usize, com_tamanho: bool) {
    if buckets.is_empty() {
        return;
    }
    println!("\n{titulo}");
    for bucket in buckets.iter().take(12) {
        let fatia = if total == 0 {
            String::new()
        } else {
            format!("  {:>5.1}%", percent(bucket.count, total))
        };
        if com_tamanho && bucket.size > 0 {
            println!(
                "  {:<28} {:>6}{fatia}  {:>10}",
                truncar(&bucket.label, 28),
                bucket.count,
                dnfm_core::format_size(bucket.size)
            );
        } else {
            println!(
                "  {:<28} {:>6}{fatia}",
                truncar(&bucket.label, 28),
                bucket.count
            );
        }
    }
}

pub async fn inspect(backend: &Arc<dyn DnfBackend>, opts: InspectOptions) -> i32 {
    let packages = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let alvo = opts.target.to_ascii_lowercase();
    let Some(pkg) = packages
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(&alvo) || p.nevra.eq_ignore_ascii_case(&alvo))
        .or_else(|| {
            packages
                .iter()
                .find(|p| p.name.to_ascii_lowercase().contains(&alvo))
        })
    else {
        eprintln!(
            "dnfm: nenhum pacote instalado corresponde a '{}'",
            opts.target
        );
        return EXIT_ERROR;
    };

    let marks = MarkSet::load_default();
    let custom = CustomCategories::load_default();
    let catalog = match super::load_catalog(backend, &packages).await {
        Ok(catalog) => catalog,
        Err(err) => {
            eprintln!("dnfm: aviso: seguindo sem metadados automáticos ({err})");
            TagCatalog::default()
        }
    };
    let details = backend.details(&pkg.nevra).await.ok();
    let upgrade = backend
        .upgrades()
        .await
        .ok()
        .and_then(|ups| ups.into_iter().find(|u| u.name() == pkg.name));
    let categoria = catalog.category_for(pkg);
    let tags = catalog.tags_for(pkg);
    let files = catalog.file_info(&pkg.name);

    if opts.json {
        let payload = inspect_json(
            pkg,
            details.as_ref(),
            upgrade.as_ref(),
            categoria.as_ref(),
            &tags,
            files,
            marks.contains(&pkg.name, &pkg.arch),
            custom.get(&pkg.name, &pkg.arch),
        );
        output::emit_json(output::envelope("inspect", payload));
        return EXIT_OK;
    }

    println!("{}", pkg.nevra);
    println!(
        "  repo {} · {} · {} · {}",
        pkg.repo_id,
        dnfm_core::format_size(pkg.size),
        pkg.reason.label(),
        pkg.arch
    );
    if let Some(detalhes) = details.as_ref() {
        if !detalhes.license.is_empty() {
            println!("  licença    {}", truncar(&detalhes.license, 72));
        }
        if !detalhes.vendor.is_empty() {
            println!("  fornecedor {}", detalhes.vendor);
        }
        if !detalhes.url.is_empty() {
            println!("  url        {}", detalhes.url);
        }
    }
    if let Some(tag) = categoria.as_ref() {
        println!("  categoria  {} ({})", tag.label, tag.source.label());
    }
    if let Some(label) = custom.get(&pkg.name, &pkg.arch) {
        println!("  própria    {label}");
    }
    if !tags.is_empty() {
        let lista: Vec<String> = tags
            .iter()
            .map(|t| format!("{} ({})", t.label, t.source.label()))
            .collect();
        println!("  tags       {}", lista.join(" · "));
    }
    if let Some(files) = files {
        println!("  arquivos   {}", files.count);
        for (dir, count) in files.dirs.iter().take(6) {
            println!("    {:<28} {:>6}", truncar(dir, 28), count);
        }
    }
    println!(
        "  marcado    {}",
        if marks.contains(&pkg.name, &pkg.arch) {
            "sim"
        } else {
            "não"
        }
    );
    if let Some(motivo) = dnfm_core::safety::is_protected(&pkg.name) {
        println!("  protegido  {motivo}");
    }
    match upgrade {
        Some(up) => println!(
            "  atualizar  {} → {} ({}, {})",
            pkg.evr,
            up.candidate.evr,
            up.severity.label(),
            dnfm_core::format_size(up.candidate.size)
        ),
        None => println!("  atualizar  em dia"),
    }
    if let Some(detalhes) = details.as_ref() {
        if !detalhes.description.trim().is_empty() {
            println!();
            for linha in detalhes.description.lines().take(8) {
                println!("  {linha}");
            }
        }
    }

    EXIT_OK
}

pub async fn tags(backend: &Arc<dyn DnfBackend>, opts: TagsOptions) -> i32 {
    let packages = match backend.installed().await {
        Ok(list) => list,
        Err(err) => return report_err(&err),
    };
    let catalog = match super::load_catalog(backend, &packages).await {
        Ok(catalog) => catalog,
        Err(err) => return report_err(&err),
    };
    let contagem = dnfm_core::metadata::tag_counts(&packages, &catalog);
    let mut lista: Vec<(String, usize)> = contagem.into_iter().collect();
    lista.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    lista.truncate(opts.limit);

    if opts.json {
        let payload = serde_json::json!({
            "tags": lista
                .iter()
                .map(|(label, count)| serde_json::json!({"tag": label, "count": count}))
                .collect::<Vec<_>>(),
            "instalados": packages.len(),
        });
        output::emit_json(output::envelope("tags", payload));
        return EXIT_OK;
    }

    println!("{} tags em {} pacotes\n", lista.len(), packages.len());
    for (label, count) in &lista {
        println!(
            "  {:<28} {:>6}  {:>5.1}%",
            truncar(label, 28),
            count,
            percent(*count, packages.len())
        );
    }
    EXIT_OK
}

fn stats_json(stats: &Stats) -> serde_json::Value {
    fn buckets(items: &[dnfm_core::Bucket]) -> serde_json::Value {
        serde_json::Value::Array(
            items
                .iter()
                .map(|b| serde_json::json!({"label": b.label, "count": b.count, "size": b.size}))
                .collect(),
        )
    }
    serde_json::json!({
        "total": stats.total,
        "unique_names": stats.unique_names,
        "total_size": stats.total_size,
        "protected": stats.protected,
        "marked": stats.marked,
        "marked_size": stats.marked_size,
        "upgrades": stats.upgrades,
        "upgrade_bytes": stats.upgrade_bytes,
        "with_appstream": stats.with_appstream,
        "with_files": stats.with_files,
        "by_category": buckets(&stats.by_category),
        "by_reason": buckets(&stats.by_reason),
        "by_repo": buckets(&stats.by_repo),
        "by_arch": buckets(&stats.by_arch),
        "by_severity": buckets(&stats.by_severity),
        "signals": buckets(&stats.signals),
        "top_tags": buckets(&stats.top_tags),
        "biggest": buckets(&stats.biggest),
        "deepest": buckets(&stats.deepest),
    })
}

#[allow(clippy::too_many_arguments)]
fn inspect_json(
    pkg: &dnfm_core::Package,
    details: Option<&dnfm_core::PackageDetails>,
    upgrade: Option<&dnfm_core::Upgrade>,
    categoria: Option<&dnfm_core::Tag>,
    tags: &[dnfm_core::Tag],
    files: Option<&dnfm_core::PackageFiles>,
    marcado: bool,
    propria: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "package": output::package_json(pkg),
        "details": details.map(|d| serde_json::json!({
            "url": d.url,
            "license": d.license,
            "vendor": d.vendor,
            "group": d.group,
            "description": d.description,
        })),
        "category": categoria.map(|t| serde_json::json!({
            "label": t.label,
            "source": t.source.as_str(),
            "confidence": t.confidence(),
        })),
        "custom_category": propria,
        "tags": tags.iter().map(|t| serde_json::json!({
            "label": t.label,
            "source": t.source.as_str(),
            "confidence": t.confidence(),
        })).collect::<Vec<_>>(),
        "files": files.map(|f| serde_json::json!({
            "count": f.count,
            "dirs": f.dirs.iter().map(|(dir, n)| serde_json::json!({"dir": dir, "count": n})).collect::<Vec<_>>(),
            "signals": f.signals.labels(),
        })),
        "marked": marcado,
        "protected": dnfm_core::safety::is_protected(&pkg.name),
        "upgrade": upgrade.map(|u| serde_json::json!({
            "candidate": u.candidate.nevra,
            "size": u.candidate.size,
            "severity": u.severity.label(),
        })),
    })
}

fn percent(parte: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        parte as f64 * 100.0 / total as f64
    }
}

fn truncar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        return texto.to_string();
    }
    let mut saida: String = texto.chars().take(max.saturating_sub(1)).collect();
    saida.push('…');
    saida
}
