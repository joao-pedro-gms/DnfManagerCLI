//! dnfm — gerenciador de pacotes DNF5 com TUI e CLI.

mod cmd;
mod output;

use std::sync::Arc;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use dnfm_core::DnfBackend;

const AFTER_HELP: &str = "\
Códigos de saída:
  0  sucesso
  1  erro
  2  problemas de resolução (nada foi executado)
  3  autenticação negada pelo polkit
  4  daemon dnf5 indisponível

Exemplos:
  dnfm                                  abre a interface
  dnfm list --pattern tex --sort size   busca e ordena
  dnfm updates --json | jq .            atualizações em JSON
  dnfm marks add vlc firefox            marca para remoção
  dnfm remove --from-marks --dry-run    prévia sem executar";

#[derive(Parser, Debug)]
#[command(
    name = "dnfm",
    version,
    about = "Gerenciador de pacotes DNF5 (TUI + CLI)",
    after_help = AFTER_HELP,
    disable_help_subcommand = true
)]
struct Cli {
    /// Usa dados de exemplo em vez do daemon (demonstração e testes)
    #[arg(long, global = true, hide = true)]
    fake: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Abre a interface de terminal
    Tui,

    /// Lista pacotes instalados
    List {
        /// Padrões de busca (nome ou trecho)
        #[arg(short, long)]
        pattern: Vec<String>,
        /// Filtra por repositório
        #[arg(short, long)]
        repo: Option<String>,
        /// Filtra por categoria (repetível)
        #[arg(long, value_name = "CATEGORIA")]
        category: Vec<String>,
        /// Filtra por família (repetível)
        #[arg(long, value_name = "FAMÍLIA")]
        family: Vec<String>,
        /// Filtra por arquitetura
        #[arg(long)]
        arch: Option<String>,
        /// Tamanho mínimo (bytes ou sufixo: 10M, 1G)
        #[arg(long, value_parser = parse_size)]
        min_size: Option<u64>,
        /// Tamanho máximo (bytes ou sufixo: 10M, 1G)
        #[arg(long, value_parser = parse_size)]
        max_size: Option<u64>,
        /// Instalados nos últimos N dias
        #[arg(long, value_name = "DIAS")]
        installed_since: Option<i64>,
        /// Só pacotes com atualização disponível
        #[arg(long)]
        with_updates: bool,
        /// Ordenação: name, size, date, repo, reason, category, family, arch
        #[arg(short, long, default_value = "name")]
        sort: String,
        /// Agrupamento: none, category, family, repo, reason
        #[arg(short = 'g', long, default_value = "none")]
        group_by: String,
        /// Limita o número de linhas
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Lista e edita categorias próprias
    Categories {
        #[command(subcommand)]
        action: Option<CategoriesAction>,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Mostra atualizações disponíveis
    Updates {
        /// Apenas atualizações de segurança (important/critical)
        #[arg(long, alias = "security")]
        security_only: bool,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Gerencia a lista de pacotes marcados para remoção
    Marks {
        #[command(subcommand)]
        action: Option<MarksAction>,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove pacotes (com prévia obrigatória)
    Remove {
        /// Pacotes a remover
        packages: Vec<String>,
        /// Usa a lista de marcações
        #[arg(long)]
        from_marks: bool,
        /// Apenas mostra a prévia
        #[arg(long)]
        dry_run: bool,
        /// Aplica a transação sem pedir confirmação
        #[arg(short = 'y', long)]
        yes: bool,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Histórico de transações
    History {
        /// Quantidade de transações
        #[arg(short, long, default_value_t = 20)]
        limit: i64,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Lista pacotes instalados como dependência que ninguém mais requer
    Orphans {
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Lista repositórios configurados
    Repos {
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },

    /// Gera script de completação de shell
    Completions {
        /// Shell alvo
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
enum CategoriesAction {
    /// Atribui uma categoria aos pacotes
    Assign {
        /// Nome da categoria (criada se não existir)
        label: String,
        /// Pacotes instalados
        packages: Vec<String>,
    },
    /// Remove a categoria própria dos pacotes
    Rm { packages: Vec<String> },
    /// Remove todas as atribuições
    Clear,
}

#[derive(Subcommand, Debug)]
enum MarksAction {
    /// Marca pacotes instalados
    Add { packages: Vec<String> },
    /// Desmarca pacotes
    Rm { packages: Vec<String> },
    /// Remove todas as marcações
    Clear,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dnfm=info,warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    // Piping para `head`/`less` não deve derrubar o processo com panic.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let code = run().await;
    std::process::exit(code);
}

async fn run() -> i32 {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Tui) => run_tui(cli.fake).await,
        Some(Command::List {
            pattern,
            repo,
            category,
            family,
            arch,
            min_size,
            max_size,
            installed_since,
            with_updates,
            sort,
            group_by,
            limit,
            json,
        }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::list(
                &backend,
                cmd::ListOptions {
                    pattern,
                    repo,
                    category,
                    family,
                    arch,
                    min_size,
                    max_size,
                    installed_since_days: installed_since,
                    with_updates,
                    sort,
                    group_by,
                    limit,
                    json,
                },
            )
            .await
        }
        Some(Command::Categories { action, json }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            let action = match action {
                None => cmd::CategoryAction::List,
                Some(CategoriesAction::Assign { label, packages }) => {
                    cmd::CategoryAction::Assign { label, packages }
                }
                Some(CategoriesAction::Rm { packages }) => cmd::CategoryAction::Remove { packages },
                Some(CategoriesAction::Clear) => cmd::CategoryAction::Clear,
            };
            cmd::categories(&backend, cmd::CategoriesOptions { action, json }).await
        }
        Some(Command::Updates {
            security_only,
            json,
        }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::updates(
                &backend,
                cmd::UpdatesOptions {
                    security_only,
                    json,
                },
            )
            .await
        }
        Some(Command::Marks { action, json }) => {
            let (add, remove, clear) = match action {
                Some(MarksAction::Add { packages }) => (packages, Vec::new(), false),
                Some(MarksAction::Rm { packages }) => (Vec::new(), packages, false),
                Some(MarksAction::Clear) => (Vec::new(), Vec::new(), true),
                None => (Vec::new(), Vec::new(), false),
            };
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::marks(
                &backend,
                cmd::MarksOptions {
                    add,
                    remove,
                    clear,
                    json,
                },
            )
            .await
        }
        Some(Command::Remove {
            packages,
            from_marks,
            dry_run,
            yes,
            json,
        }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::remove(
                &backend,
                cmd::RemoveOptions {
                    specs: packages,
                    from_marks,
                    dry_run,
                    yes,
                    json,
                },
            )
            .await
        }
        Some(Command::History { limit, json }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::history(&backend, cmd::HistoryOptions { limit, json }).await
        }
        Some(Command::Orphans { json }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::orphans(&backend, json).await
        }
        Some(Command::Completions { shell }) => {
            let mut command = Cli::command();
            clap_complete::generate(shell, &mut command, "dnfm", &mut std::io::stdout());
            cmd::EXIT_OK
        }
        Some(Command::Repos { json }) => {
            let backend = match connect(cli.fake).await {
                Ok(backend) => backend,
                Err(err) => return cmd::report_err(&err),
            };
            cmd::repos(&backend, json).await
        }
    }
}

/// Aceita `1024`, `10K`, `10M`, `1G` (base 1024).
fn parse_size(raw: &str) -> Result<u64, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("tamanho vazio".to_string());
    }
    let (number, multiplier) = match text.chars().last().unwrap().to_ascii_lowercase() {
        'k' => (&text[..text.len() - 1], 1024u64),
        'm' => (&text[..text.len() - 1], 1024 * 1024),
        'g' => (&text[..text.len() - 1], 1024 * 1024 * 1024),
        't' => (&text[..text.len() - 1], 1024u64 * 1024 * 1024 * 1024),
        _ => (text, 1),
    };
    let value: f64 = number
        .trim()
        .parse()
        .map_err(|_| format!("tamanho inválido: {raw}"))?;
    Ok((value * multiplier as f64) as u64)
}

async fn connect(fake: bool) -> Result<Arc<dyn DnfBackend>, dnfm_core::BackendError> {
    if fake {
        Ok(dnfm_core::fake::FakeBackend::shared())
    } else {
        let backend = dnfm_core::dbus::DbusBackend::connect().await?;
        Ok(backend)
    }
}

async fn run_tui(fake: bool) -> i32 {
    let backend = match connect(fake).await {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("dnfm: {err}");
            eprintln!("dnfm: verifique o daemon com `systemctl status dnf5daemon-server`");
            return cmd::exit_code(&err);
        }
    };
    match dnfm_tui::run(backend).await {
        Ok(_) => cmd::EXIT_OK,
        Err(err) => {
            eprintln!("dnfm: {err}");
            cmd::EXIT_ERROR
        }
    }
}
