//! Formatação de saída: tabela para humanos, JSON estável para scripts.

use dnfm_core::{format_date, format_size, HistoryEntry, Package, Repo, Severity, Upgrade};

pub const SCHEMA_VERSION: u32 = 1;

/// Imprime uma tabela alinhada; a última coluna não é truncada.
pub fn table(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx < widths.len() {
                widths[idx] = widths[idx].max(cell.chars().count());
            }
        }
    }
    let header = headers
        .iter()
        .enumerate()
        .map(|(i, h)| pad(h, widths[i]))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{}", header.trim_end());
    println!(
        "{}",
        widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i + 1 == row.len() {
                    cell.clone()
                } else {
                    pad(cell, widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("{}", line.trim_end());
    }
}

fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - len))
    }
}

pub fn package_json(pkg: &Package) -> serde_json::Value {
    serde_json::json!({
        "name": pkg.name,
        "evr": pkg.evr,
        "arch": pkg.arch,
        "nevra": pkg.nevra,
        "repo_id": pkg.repo_id,
        "from_repo_id": pkg.from_repo_id,
        "install_size": pkg.size,
        "install_date": format_date(pkg.install_time),
        "reason": pkg.reason.key(),
        "summary": pkg.summary,
    })
}

pub fn package_row(pkg: &Package, marked: bool, category: &str) -> Vec<String> {
    vec![
        if marked { "x" } else { " " }.to_string(),
        pkg.name.clone(),
        pkg.evr.clone(),
        pkg.arch.clone(),
        pkg.repo_id.clone(),
        format_size(pkg.size),
        format_date(pkg.install_time),
        category.to_string(),
        pkg.reason.label().to_string(),
    ]
}

pub const PACKAGE_HEADERS: [&str; 9] = [
    "M",
    "nome",
    "versão",
    "arch",
    "repo",
    "tamanho",
    "instalado",
    "categoria",
    "motivo",
];

pub fn upgrade_json(up: &Upgrade) -> serde_json::Value {
    serde_json::json!({
        "name": up.installed.name,
        "installed_evr": up.installed.evr,
        "available_evr": up.candidate.evr,
        "arch": up.installed.arch,
        "repo_id": up.candidate.repo_id,
        "severity": up.severity.key(),
        "severity_rank": severity_rank(up.severity),
        "advisory": up.advisory,
        "download_size": up.candidate.size,
        "summary": up.candidate.summary,
    })
}

pub fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Unknown => 0,
        Severity::Low => 1,
        Severity::Moderate => 2,
        Severity::Important => 3,
        Severity::Critical => 4,
    }
}

pub fn history_json(entry: &HistoryEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "description": entry.description,
        "status": entry.status,
        "date": format_date(entry.start),
        "user_id": entry.user_id,
        "installed": entry.installed,
        "removed": entry.removed,
        "upgraded": entry.upgraded,
        "downgraded": entry.downgraded,
    })
}

pub fn repo_json(repo: &Repo) -> serde_json::Value {
    serde_json::json!({
        "id": repo.id,
        "name": repo.name,
        "enabled": repo.enabled,
    })
}

pub fn emit_json(value: serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
    );
}

pub fn envelope(command: &str, payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "generated_at": dnfm_core::marks::now_unix(),
        "data": payload,
    })
}
