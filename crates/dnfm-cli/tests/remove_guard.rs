//! Testes de ponta a ponta da blocklist na remoção.
//!
//! O binário roda de verdade com o backend `--fake`: pedir a remoção de um
//! protegido tem de ser recusado com código 2 antes de qualquer prévia.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Diretório XDG isolado por teste.
fn base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dnfm-remove-guard-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("state")).expect("criar XDG isolado");
    dir
}

/// Executa o `dnfm` e devolve (código de saída, stdout, stderr).
fn dnfm(base: &Path, args: &[&str]) -> (i32, String, String) {
    let saida = Command::new(env!("CARGO_BIN_EXE_dnfm"))
        .args(args)
        .env("XDG_STATE_HOME", base.join("state"))
        .env("XDG_CACHE_HOME", base.join("cache"))
        .env("XDG_CONFIG_HOME", base.join("config"))
        .output()
        .expect("executar o binário dnfm");
    (
        saida
            .status
            .code()
            .expect("o processo terminou por sinal, não por código"),
        String::from_utf8_lossy(&saida.stdout).into_owned(),
        String::from_utf8_lossy(&saida.stderr).into_owned(),
    )
}

#[test]
fn remove_protegido_recusa_com_codigo_2() {
    let dir = base("protegido");
    let (codigo, stdout, stderr) = dnfm(&dir, &["--fake", "remove", "dnf5", "--yes"]);
    assert_eq!(codigo, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("dnfm: dnf5 é protegido"),
        "falta a recusa no stderr: {stderr}"
    );
    // Recusa antes da prévia: nada sai no stdout.
    assert!(stdout.is_empty(), "stdout: {stdout}");
}

#[test]
fn remove_comum_com_dry_run_continua_em_zero() {
    let dir = base("comum");
    let (codigo, stdout, stderr) =
        dnfm(&dir, &["--fake", "remove", "firefox", "--yes", "--dry-run"]);
    assert_eq!(codigo, 0, "stderr: {stderr}");
    assert!(stdout.contains("você pediu"), "stdout: {stdout}");
}

#[test]
fn remove_de_marcas_com_protegido_recusa() {
    let dir = base("marcas");
    // Marcação que escapou do bloqueio (arquivo editado à mão).
    fs::create_dir_all(dir.join("state/dnfm")).expect("criar dir de estado");
    fs::write(
        dir.join("state/dnfm/marks.json"),
        r#"{
  "version": 1,
  "marks": [
    { "name": "sudo", "arch": "x86_64", "nevra": "sudo-1.0-1.fc45.x86_64" },
    { "name": "vlc", "arch": "x86_64", "nevra": "vlc-1.0-1.fc45.x86_64" }
  ]
}"#,
    )
    .expect("gravar marks.json de teste");
    let (codigo, stdout, stderr) = dnfm(&dir, &["--fake", "remove", "--from-marks", "--yes"]);
    assert_eq!(codigo, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("dnfm: sudo é protegido"),
        "falta a recusa do protegido das marcas: {stderr}"
    );
    assert!(stdout.is_empty(), "stdout: {stdout}");
}
