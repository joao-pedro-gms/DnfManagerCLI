//! Testes de ponta a ponta do JSON do `list`.
//!
//! O binário é executado de verdade com o backend `--fake`, então o teste cobre
//! o contrato publicado para scripts, não só a função interna.

use std::process::Command;

/// Executa o `dnfm` com XDG isolado e devolve o stdout.
fn dnfm(args: &[&str]) -> String {
    let base = std::env::temp_dir().join(format!("dnfm-cli-teste-{}", std::process::id()));
    let saida = Command::new(env!("CARGO_BIN_EXE_dnfm"))
        .args(args)
        .env("XDG_STATE_HOME", base.join("state"))
        .env("XDG_CACHE_HOME", base.join("cache"))
        .env("XDG_CONFIG_HOME", base.join("config"))
        .output()
        .expect("executar o binário dnfm");
    assert!(
        saida.status.success(),
        "dnfm {args:?} saiu com {:?}: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    String::from_utf8(saida.stdout).expect("stdout em UTF-8")
}

fn json(args: &[&str]) -> serde_json::Value {
    let saida = dnfm(args);
    serde_json::from_str(&saida).unwrap_or_else(|e| panic!("stdout não é JSON ({e}):\n{saida}"))
}

#[test]
fn list_json_com_limite_informa_total_e_retornadas() {
    // O backend fake tem 6 instalados; `-n 2` corta a saída, não a contagem.
    let payload = json(&["--fake", "list", "-n", "2", "--json"]);
    assert_eq!(payload["command"], "list");

    let data = &payload["data"];
    assert_eq!(data["total"].as_u64(), Some(6), "total antes do limite");
    assert_eq!(data["returned"].as_u64(), Some(2), "pacotes devolvidos");
    assert_eq!(
        data["packages"].as_array().map(Vec::len),
        Some(2),
        "o array acompanha o limite"
    );
}

#[test]
fn list_json_sem_limite_devolve_o_total() {
    let payload = json(&["--fake", "list", "--json"]);
    let data = &payload["data"];
    assert_eq!(data["total"].as_u64(), Some(6));
    assert_eq!(data["returned"].as_u64(), Some(6));
    assert_eq!(data["packages"].as_array().map(Vec::len), Some(6));
}

#[test]
fn list_json_com_filtro_conta_os_casados() {
    // `--pattern` filtra antes do limite: total é o que casou, returned é o que saiu.
    let payload = json(&[
        "--fake",
        "list",
        "--pattern",
        "firefox",
        "-n",
        "1",
        "--json",
    ]);
    let data = &payload["data"];
    assert_eq!(data["total"].as_u64(), Some(1));
    assert_eq!(data["returned"].as_u64(), Some(1));
}
