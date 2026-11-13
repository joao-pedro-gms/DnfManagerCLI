# dnfm

Gerenciador de pacotes DNF5 com interface de terminal (TUI) e linha de comando,
feito para o Fedora. Marca pacotes para remoção em lote, mostra o que sai junto
como dependência **antes** de aplicar, e acompanha a transação em tempo real.

Fala com o `dnf5daemon` por D-Bus: não reimplementa o resolvedor do DNF e não
precisa de root. As operações de escrita passam pelo polkit, como qualquer
ferramenta gráfica do sistema.

```
┌─ dnfm · 8008 instalados · 48 atualizações · 3 marcados ─────────────────────┐
│ [Instalados]  Atualizações  Órfãos  Histórico  Repos                        │
├─────────────────────────────────────────────────────────────────────────────┤
│  [ ]  nome               versão              repo    tamanho  instalado     │
│  [x]  dosbox-staging     0.82.2-6.fc45       fedora  16 MiB   03/10         │
│  [!]  glibc              2.42-3.fc45         fedora  12 MiB   03/10         │
├─────────────────────────────────────────────────────────────────────────────┤
│ dosbox-staging — DOSBox Staging is a modern continuation of DOSBox…         │
│ licença: GPL-2.0-or-later   https://www.dosbox-staging.org                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ espaço marca · p preview · / busca · s ordena · tab abas · ? ajuda · q sai  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Instalação

```bash
# a partir do RPM gerado
sudo dnf install ./dnfm-0.1.0-1.fc45.x86_64.rpm

# ou direto do código
cargo build --release
install -Dm755 target/release/dnfm ~/.local/bin/dnfm
```

Requisitos: Fedora com `dnf5` (o daemon `dnf5daemon-server` é ativado sob
demanda) e um agente polkit (`polkit-kde` ou `polkit-gnome`) para transações.

## Uso

```bash
dnfm                                    # interface
dnfm list --pattern texlive --sort size # listar e filtrar
dnfm updates --security-only --json     # atualizações de segurança
dnfm marks add vlc firefox              # marcar para remoção
dnfm remove --from-marks --dry-run      # ver o efeito, sem aplicar
dnfm remove --from-marks --yes          # aplicar (polkit pede a senha)
dnfm orphans                            # dependências que ninguém mais requer
dnfm history --limit 10                 # histórico de transações
```

### Teclas

| tecla | ação |
|---|---|
| `espaço` | marca/desmarca o pacote sob o cursor |
| `a` / `u` | marca todos os visíveis / limpa marcações |
| `p` | preview da remoção (marcados ou o atual) |
| `P` | preview da atualização selecionada |
| `enter` | detalhes (ou seleciona atualização) |
| `!` / `A` | seleciona atualizações de segurança / todas |
| `/` | busca fuzzy por nome e resumo |
| `s` / `S` | cicla ordenação / inverte |
| `tab` | próxima aba |
| `r` | recarrega |
| `?` | ajuda |

Mouse: clique seleciona, clique na caixa marca, roda rola, clique no cabeçalho
ordena, clique na aba troca de aba.

## Segurança

- **Preview obrigatório.** O tipo `ConfirmToken` só é construído por
  `Preview::confirm()`, então não existe caminho de código que execute uma
  transação sem ter passado por um preview aprovado.
- **Blocklist de essenciais.** `dnf5`, `glibc`, `systemd`, kernel, `sudo`,
  `rpm`, `polkit`, `grub2`, `dracut` e afins não podem ser marcados.
- **O resolvedor é a rede final.** Se o DNF reportar qualquer problema de
  resolução, a execução é recusada — cobre dependências que a lista estática
  não conhece.
- **Marcações em arquivo.** `~/.local/state/dnfm/marks.json`, gravação atômica,
  arquivo corrompido vira `marks.json.corrupt` em vez de sumir.

## Arquitetura

```
crates/dnfm-core   domínio + backend D-Bus (zbus) + marcas + blocklist + busca
crates/dnfm-tui    ratatui + crossterm; estado sem I/O, efeitos como Cmd
crates/dnfm-cli    clap; binário `dnfm`
```

O backend é um trait (`DnfBackend`), com implementação real em D-Bus e uma
implementação em memória usada pelos testes (inclusive cenários de erro: polkit
negado, resolução bloqueada, scriptlet falhando).

## Desenvolvimento

```bash
cargo test --workspace            # 46 testes, sem tocar no sistema
cargo clippy --workspace -- -D warnings
cargo fmt --check

# testes contra o daemon real (somente leitura + resolve, nunca executa)
cargo test -p dnfm-core --test real_daemon -- --ignored --nocapture
```

Fixtures reais do daemon ficam em `crates/dnfm-core/tests/fixtures/` e são
recapturadas com `scripts/capture_fixtures.py`.

## Empacotamento

```bash
cargo build --release
mkdir -p ~/rpmbuild/{SOURCES,SPECS}
cp target/release/dnfm packaging/dnfm.1 packaging/dnfm.bash packaging/dnfm.zsh packaging/dnfm.fish ~/rpmbuild/SOURCES/
cp packaging/dnfm.spec ~/rpmbuild/SPECS/
rpmbuild -ba ~/rpmbuild/SPECS/dnfm.spec
```

## Licença

MIT.
