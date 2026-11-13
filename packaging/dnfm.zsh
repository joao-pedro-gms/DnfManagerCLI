#compdef dnfm

autoload -U is-at-least

_dnfm() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_dnfm_commands" \
"*::: :->dnfm" \
&& ret=0
    case $state in
    (dnfm)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:dnfm-command-$line[1]:"
        case $line[1] in
            (tui)
_arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
'*-p+[Padrões de busca (nome ou trecho)]:PATTERN:_default' \
'*--pattern=[Padrões de busca (nome ou trecho)]:PATTERN:_default' \
'-r+[Filtra por repositório]:REPO:_default' \
'--repo=[Filtra por repositório]:REPO:_default' \
'-s+[Ordenação\: name, size, date, repo, reason]:SORT:_default' \
'--sort=[Ordenação\: name, size, date, repo, reason]:SORT:_default' \
'-n+[Limita o número de linhas]:LIMIT:_default' \
'--limit=[Limita o número de linhas]:LIMIT:_default' \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(updates)
_arguments "${_arguments_options[@]}" : \
'--security-only[Apenas atualizações de segurança (important/critical)]' \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(marks)
_arguments "${_arguments_options[@]}" : \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
":: :_dnfm__subcmd__marks_commands" \
"*::: :->marks" \
&& ret=0

    case $state in
    (marks)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:dnfm-marks-command-$line[1]:"
        case $line[1] in
            (add)
_arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
'*::packages:_default' \
&& ret=0
;;
(rm)
_arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
'*::packages:_default' \
&& ret=0
;;
(clear)
_arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
        esac
    ;;
esac
;;
(remove)
_arguments "${_arguments_options[@]}" : \
'--from-marks[Usa a lista de marcações]' \
'--dry-run[Apenas mostra a prévia]' \
'-y[Aplica a transação sem pedir confirmação]' \
'--yes[Aplica a transação sem pedir confirmação]' \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
'*::packages -- Pacotes a remover:_default' \
&& ret=0
;;
(history)
_arguments "${_arguments_options[@]}" : \
'-l+[Quantidade de transações]:LIMIT:_default' \
'--limit=[Quantidade de transações]:LIMIT:_default' \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(orphans)
_arguments "${_arguments_options[@]}" : \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(repos)
_arguments "${_arguments_options[@]}" : \
'--json[Saída em JSON]' \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" : \
'--fake[Usa dados de exemplo em vez do daemon (demonstração e testes)]' \
'-h[Print help]' \
'--help[Print help]' \
':shell -- Shell alvo:(bash elvish fish powershell zsh)' \
&& ret=0
;;
        esac
    ;;
esac
}

(( $+functions[_dnfm_commands] )) ||
_dnfm_commands() {
    local commands; commands=(
'tui:Abre a interface de terminal' \
'list:Lista pacotes instalados' \
'updates:Mostra atualizações disponíveis' \
'marks:Gerencia a lista de pacotes marcados para remoção' \
'remove:Remove pacotes (com prévia obrigatória)' \
'history:Histórico de transações' \
'orphans:Lista pacotes instalados como dependência que ninguém mais requer' \
'repos:Lista repositórios configurados' \
'completions:Gera script de completação de shell' \
    )
    _describe -t commands 'dnfm commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__completions_commands] )) ||
_dnfm__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm completions commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__history_commands] )) ||
_dnfm__subcmd__history_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm history commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__list_commands] )) ||
_dnfm__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm list commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__marks_commands] )) ||
_dnfm__subcmd__marks_commands() {
    local commands; commands=(
'add:Marca pacotes instalados' \
'rm:Desmarca pacotes' \
'clear:Remove todas as marcações' \
    )
    _describe -t commands 'dnfm marks commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__marks__subcmd__add_commands] )) ||
_dnfm__subcmd__marks__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm marks add commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__marks__subcmd__clear_commands] )) ||
_dnfm__subcmd__marks__subcmd__clear_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm marks clear commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__marks__subcmd__rm_commands] )) ||
_dnfm__subcmd__marks__subcmd__rm_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm marks rm commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__orphans_commands] )) ||
_dnfm__subcmd__orphans_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm orphans commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__remove_commands] )) ||
_dnfm__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm remove commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__repos_commands] )) ||
_dnfm__subcmd__repos_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm repos commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__tui_commands] )) ||
_dnfm__subcmd__tui_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm tui commands' commands "$@"
}
(( $+functions[_dnfm__subcmd__updates_commands] )) ||
_dnfm__subcmd__updates_commands() {
    local commands; commands=()
    _describe -t commands 'dnfm updates commands' commands "$@"
}

if [ "$funcstack[1]" = "_dnfm" ]; then
    _dnfm "$@"
else
    compdef _dnfm dnfm
fi
