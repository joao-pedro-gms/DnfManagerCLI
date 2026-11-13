//! Testes contra o `dnf5daemon` real.
//!
//! Ficam atrás de `#[ignore]` porque dependem do daemon rodando e do estado da
//! máquina. Nenhum deles executa transação: só leitura e `resolve`.
//!
//!     cargo test -p dnfm-core --test real_daemon -- --ignored --nocapture

use dnfm_core::dbus::DbusBackend;
use dnfm_core::DnfBackend;

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn lista_pacotes_instalados() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let installed = backend.installed().await.expect("listar instalados");
    println!("instalados: {}", installed.len());
    assert!(!installed.is_empty(), "sistema sem pacotes instalados?");
    assert!(installed.iter().all(|p| p.is_installed));
    assert!(installed
        .iter()
        .any(|p| !p.name.is_empty() && !p.evr.is_empty()));
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn lista_atualizacoes_com_versao_nova() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let upgrades = backend.upgrades().await.expect("listar atualizações");
    println!("atualizações: {}", upgrades.len());
    for up in upgrades.iter().take(5) {
        println!(
            "  {}: {} -> {} ({:?})",
            up.name(),
            up.installed.evr,
            up.candidate.evr,
            up.severity
        );
    }
    for up in &upgrades {
        assert_ne!(
            up.installed.evr,
            up.candidate.evr,
            "{} apareceu como atualização sem mudar de versão",
            up.name()
        );
        assert!(!up.candidate.is_installed);
    }
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn preview_de_remocao_separa_pedido_de_dependencia() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let installed = backend.installed().await.expect("listar instalados");

    // Escolhe um pacote folha: instalado pelo usuário, pequeno e sem nada
    // depender dele (heurística: tenta até achar um preview limpo).
    let candidates: Vec<String> = installed
        .iter()
        .filter(|p| p.reason.is_user() && p.name.len() > 3)
        .take(5)
        .map(|p| p.name.clone())
        .collect();

    let mut found = None;
    for name in candidates {
        let preview = backend.preview_remove(std::slice::from_ref(&name)).await;
        if let Ok(preview) = preview {
            if !preview.is_failed() {
                found = Some((name, preview));
                break;
            }
        }
    }
    let Some((target, preview)) = found else {
        println!("nenhum candidato removível sem quebrar dependências; pulando");
        return;
    };

    println!(
        "remove {}: {} itens, {} liberados",
        target,
        preview.items.len(),
        dnfm_core::format_size(preview.freed_bytes())
    );
    assert!(preview.requested().iter().any(|i| i.package.name == target));
    for item in preview.collateral() {
        println!(
            "  arrastado: {} ({})",
            item.package.name,
            item.reason.label()
        );
    }
    assert!(preview.confirm().is_ok(), "preview limpo deveria confirmar");
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn removao_de_pacote_critico_e_bloqueada_pelo_solver() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let preview = backend.preview_remove(&["glibc".to_string()]).await;
    match preview {
        Ok(preview) => {
            println!("problemas: {:?}", preview.problems);
            assert!(
                preview.is_failed(),
                "remover glibc deveria falhar no solver"
            );
            assert!(
                preview.confirm().is_err(),
                "não pode virar token de execução"
            );
        }
        Err(err) => println!("daemon recusou antes do resolve: {err}"),
    }
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn historico_e_repos() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let history = backend.history(3).await.expect("histórico");
    println!("transações: {}", history.len());
    for entry in &history {
        println!("  #{} {} ({})", entry.id, entry.description, entry.status);
        assert!(entry.id > 0);
    }
    let repos = backend.repos().await.expect("repos");
    println!("repos: {}", repos.len());
    assert!(repos
        .iter()
        .any(|r| r.id == "fedora" || r.id.starts_with("updates")));
}

#[tokio::test]
#[ignore = "requer dnf5daemon-server ativo"]
async fn detalhes_de_um_pacote() {
    let backend = DbusBackend::connect().await.expect("conectar no daemon");
    let installed = backend.installed().await.expect("listar");
    let first = installed
        .iter()
        .find(|p| p.name == "bash" || p.name == "coreutils")
        .or_else(|| installed.first())
        .expect("algum pacote")
        .clone();
    let details = backend.details(&first.nevra).await.expect("detalhes");
    println!(
        "{}: {}",
        first.name,
        details.description.chars().take(80).collect::<String>()
    );
    assert!(!details.nevra.is_empty());
}
