# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_dnfm_global_optspecs
    string join \n fake h/help V/version
end

function __fish_dnfm_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_dnfm_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_dnfm_using_subcommand
    set -l cmd (__fish_dnfm_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c dnfm -n "__fish_dnfm_needs_command" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_needs_command" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_needs_command" -s V -l version -d 'Print version'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "tui" -d 'Abre a interface de terminal'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "list" -d 'Lista pacotes instalados'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "categories" -d 'Lista e edita categorias próprias'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "updates" -d 'Mostra atualizações disponíveis'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "marks" -d 'Gerencia a lista de pacotes marcados para remoção'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "remove" -d 'Remove pacotes (com prévia obrigatória)'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "history" -d 'Histórico de transações'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "orphans" -d 'Lista pacotes instalados como dependência que ninguém mais requer'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "repos" -d 'Lista repositórios configurados'
complete -c dnfm -n "__fish_dnfm_needs_command" -f -a "completions" -d 'Gera script de completação de shell'
complete -c dnfm -n "__fish_dnfm_using_subcommand tui" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand tui" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s p -l pattern -d 'Padrões de busca (nome ou trecho)' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s r -l repo -d 'Filtra por repositório' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l category -d 'Filtra por categoria (repetível)' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l family -d 'Filtra por família (repetível)' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l arch -d 'Filtra por arquitetura' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l min-size -d 'Tamanho mínimo (bytes ou sufixo: 10M, 1G)' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l max-size -d 'Tamanho máximo (bytes ou sufixo: 10M, 1G)' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l installed-since -d 'Instalados nos últimos N dias' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s s -l sort -d 'Ordenação: name, size, date, repo, reason, category, family, arch' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s g -l group-by -d 'Agrupamento: none, category, family, repo, reason' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s n -l limit -d 'Limita o número de linhas' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l with-updates -d 'Só pacotes com atualização disponível'
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand list" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -f -a "assign" -d 'Atribui uma categoria aos pacotes'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -f -a "rm" -d 'Remove a categoria própria dos pacotes'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and not __fish_seen_subcommand_from assign rm clear" -f -a "clear" -d 'Remove todas as atribuições'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from assign" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from assign" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from rm" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from rm" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from clear" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand categories; and __fish_seen_subcommand_from clear" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand updates" -l security-only -d 'Apenas atualizações de segurança (important/critical)'
complete -c dnfm -n "__fish_dnfm_using_subcommand updates" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand updates" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand updates" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -f -a "add" -d 'Marca pacotes instalados'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -f -a "rm" -d 'Desmarca pacotes'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and not __fish_seen_subcommand_from add rm clear" -f -a "clear" -d 'Remove todas as marcações'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from add" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from rm" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from rm" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from clear" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand marks; and __fish_seen_subcommand_from clear" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -l from-marks -d 'Usa a lista de marcações'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -l dry-run -d 'Apenas mostra a prévia'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -s y -l yes -d 'Aplica a transação sem pedir confirmação'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand remove" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand history" -s l -l limit -d 'Quantidade de transações' -r
complete -c dnfm -n "__fish_dnfm_using_subcommand history" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand history" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand history" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand orphans" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand orphans" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand orphans" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand repos" -l json -d 'Saída em JSON'
complete -c dnfm -n "__fish_dnfm_using_subcommand repos" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand repos" -s h -l help -d 'Print help'
complete -c dnfm -n "__fish_dnfm_using_subcommand completions" -l fake -d 'Usa dados de exemplo em vez do daemon (demonstração e testes)'
complete -c dnfm -n "__fish_dnfm_using_subcommand completions" -s h -l help -d 'Print help'
