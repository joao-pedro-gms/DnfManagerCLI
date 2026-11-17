# dnfm

Gerenciador de pacotes DNF5 com interface de terminal (TUI) e linha de comando,
feito para o Fedora. Marca pacotes para remoção em lote, mostra o que sai junto
como dependência **antes** de aplicar, e acompanha a transação em tempo real.

Fala com o `dnf5daemon` por D-Bus: não reimplementa o resolvedor do DNF e não
precisa de root. As operações de escrita passam pelo polkit, como qualquer
ferramenta gráfica do sistema.

```
┌ dnfm   8009 pacotes · 48 atualizações · 3 marcados · 30 categorias ─────────┐
│ Instalados  Atualizações  Órfãos  Histórico  Repos      arch: x86_64 ×      │
├ Categorias ──────┬ Instalados · 8009 · por categoria ─────────────────────┤
│ TeX Live    ███ 9 │ ▼ TeX Live                    9 · 39%                 │
│ KDE Plasma  █   3 │ ▢ texlive-a2ping   svn15878  @System  484 KiB  TeX Live│
│ Outros      █   3 │ ▢ texlive-ae       svn15878  @System  484 KiB  TeX Live│
│ total       23    │ ▼ KDE Plasma                  3 · 13%                 │
│ marcados     2    │ ▢ kf6-kcoreaddons  6.8.0-1   @System  2.1 MiB KDE…     │
├ detalhes ────────┴─────────────────────────────────────────────────────────┤
│ texlive-a2ping  12:svn15878-5.fc45 · x86_64                                │
│ enter: descrição, licença e URL                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ ● 3 marcados · pronto   ordenar: nome · grupo: categoria · filtros: 1      │
│ ␣ marcar  t categoria  f filtros  o agrupar  [ ] grupos  p preview  ? ajuda │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Instalação

```bash
# a partir do RPM gerado
sudo dnf install ./dnfm-0.3.0-1.fc45.x86_64.rpm

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

dnfm stats                              # estatísticas agregadas do sistema
dnfm inspect vlc                        # metadados, tags e arquivos de um pacote
dnfm inspect vlc --json | jq .tags      # idem, em JSON
dnfm tags --limit 20                    # tags automáticas e contagens

dnfm categories                         # categorias com contagem
dnfm categories assign Multimídia vlc firefox
dnfm list --category "TeX Live" --sort size
dnfm list --group-by category --with-updates
dnfm list --arch noarch --min-size 10M --group-by family
```

`dnfm orphans` monta o grafo de dependências localmente a partir dos
`requires`, `provides` e arquivos de cada pacote instalado: uma dependência
de arquivo (`/usr/bin/ruby`) segura quem é dono do caminho, e rich deps como
`(a if b else c)` contam todas as alternativas. Pacotes protegidos ficam
fora da lista e a contagem aparece como `N protegidos ignorados`. Como a
consulta pede a lista de arquivos ao daemon, leva cerca de 20 s.

Em `--json`, `orphans` devolve um objeto com `count`, `protected_skipped` e
`packages` (não mais uma lista), e `list` informa `total` (pacotes casados)
e `returned` (impressos após o `--limit`). Todos os comandos em JSON usam o
mesmo envelope com `schema_version: 2`.

### Teclas

| tecla | ação |
|---|---|
| `espaço` | marca/desmarca o pacote sob o cursor |
| `a` / `u` | marca todos os visíveis / limpa marcações |
| `p` | preview da remoção (marcados ou o atual) |
| `P` | preview da atualização selecionada |
| `enter` / `i` | inspetor do pacote (metadados, tags, arquivos, estado) |
| `!` / `A` | seleciona atualizações de segurança / todas |
| `/` | busca fuzzy por nome e resumo |
| `o` / `O` | cicla o agrupamento (categoria, família, repo, motivo) |
| `[` / `]` | percorre os grupos do sidebar aplicando o filtro |
| `0` | limpa o filtro do grupo atual |
| `ctrl+b` | mostra/oculta o sidebar |
| `F2` | troca a paleta (midnight, solar, mono) |
| `f` | painel de filtros (texto, categoria, repo, motivo, arch, tamanho, data) |
| `c` / `C` | filtra por categoria / família (seletor com busca) |
| `t` / `T` | dá ou remove uma categoria própria |
| `s` / `S` | cicla ordenação / inverte |
| `m` (no inspetor) | fecha e marca o pacote |
| `tab` | próxima aba |
| `r` | recarrega |
| `?` | ajuda |

Mouse: clique seleciona, clique na caixa marca, roda rola, clique no cabeçalho
ordena, clique na aba troca de aba.



## Interface

O layout tem quatro zonas fixas: cabeçalho (marca, contadores, abas e **chips de
filtro clicáveis**), sidebar de grupos, tabela agrupada e painel de detalhes,
com uma keybar contextual no rodapé.

- **Sidebar** — mostra todos os grupos da dimensão atual com contagem e barra
  proporcional, além de um resumo (total, marcados, maior grupo, filtros
  ativos). Clicar num grupo filtra; `[`/`]` percorrem os grupos aplicando o
  filtro na hora; `0` limpa. Some sozinho em terminais com menos de 92 colunas.
- **Chips** — cada filtro ativo vira um chip no cabeçalho; clicar remove só
  aquele filtro, sem abrir o painel.
- **Agrupamento** — cabeçalhos de grupo com contagem, percentual e barra;
  `espaço` recolhe, `a` marca o grupo inteiro.
- **Animações** — abertura de modal com escala e sombra, barras do sidebar
  crescendo na carga, pulso no grupo que mudou e spinner durante `resolve`.
  O laço redesenha a 60 fps somente enquanto há animação; parado, cai para 5 fps.
- **Tema** — três paletas com a mesma hierarquia (destaque, texto, dim,
  superfícies, estados). `F2` cicla e a escolha fica em
  `~/.local/state/dnfm/ui.json`.
- **Mouse** — hover realça a linha, clique seleciona, clique na caixa marca,
  clique no cabeçalho ordena, clique na aba troca de aba, clique no chip remove
  o filtro e clique no sidebar filtra pelo grupo.

## Metadados automáticos

Categoria e tags vêm de quatro fontes, sem nada para o usuário preencher:

| fonte | o que dá | cobertura medida |
|---|---|---|
| **AppStream** (`/usr/share/swcatalog/xml`) | categoria funcional oficial (`AudioVideo`, `Graphics`…) | 2,2% (177 de 8.009), que são os apps de GUI |
| **arquivos `.desktop`** | mesma taxonomia para app fora do catálogo | poucos casos residuais |
| **lista de arquivos** | tipo do pacote: comando, biblioteca, man, documentação, fonte, serviço, plugin | 100% |
| **domínio da URL** | família de projeto (`tug.org` → TeX Live, `invent.kde.org` → KDE Plasma) | domínios conhecidos |

A categoria de um pacote é escolhida nesta ordem: **categoria própria** do
usuário > **AppStream** > **regra curada** por prefixo/palavra-chave > *Outros*.
A família prefere o **domínio da URL** e só então o prefixo.

As listas de arquivos custam ~3,5 s para 8.009 pacotes e o catálogo AppStream
tem 8,4 MB comprimidos, então o resultado é gravado em
`~/.cache/dnfm/metadata.json` e revalidado por uma impressão digital dos NEVRA
instalados: paga-se uma vez por transação, não por sessão (`r` força o
recálculo). Cada categoria/tag carrega a origem e a confiança, mostradas no
inspetor.

## Categorias, agrupamento e filtros

O tag `Group` do RPM está morto no Fedora (quase tudo vem como "Unspecified") e
o comps cobre uma fração mínima da base instalada, então a taxonomia é montada
em três camadas, da mais forte para a mais fraca:

1. **categoria própria** — o que você atribuiu com `t` ou `dnfm categories
   assign`. Vence sempre e fica em `~/.local/state/dnfm/categories.json`.
2. **categoria curada** — regras por prefixo e palavra-chave: `texlive*` →
   TeX Live, `kf5/kf6/plasma*` → KDE Plasma, `*-firmware` → Firmware,
   `*-devel` → Desenvolvimento, `lib*`/`*-libs` → Bibliotecas, e assim por
   diante (30 categorias na instalação padrão).
3. **família** — prefixo do nome com apelidos que juntam parentes; famílias com
   menos de 3 pacotes caem em "Outros".

Agrupamento (`o` na TUI, `--group-by` no CLI) funciona por categoria, família,
repo ou motivo. Cabeçalhos de grupo aparecem como linhas próprias: `espaço`
recolhe/expande e `a` marca o grupo inteiro.

Filtros combinam livremente (`f` abre o painel; todos são E lógico, exceto
categorias/famílias, que são OU dentro da mesma dimensão):

| filtro | TUI | CLI |
|---|---|---|
| texto (fuzzy) | `/` | `--pattern` |
| categoria / família | `c` / `C` | `--category` / `--family` |
| repo | painel `f` | `--repo` |
| motivo (usuário, dependência…) | painel `f` | — |
| arquitetura | painel `f` | `--arch` |
| tamanho (presets >100 MiB, >10 MiB, >1 MiB, <1 MiB) | painel `f` | `--min-size` / `--max-size` |
| data de instalação (7/30/90 dias, este ano) | painel `f` | `--installed-since` |
| só marcados / protegidos | painel `f` | — |
| com atualização | painel `f` | `--with-updates` |

Ordenação (`s` / `--sort`): nome, tamanho, data, repo, motivo, categoria,
família, arquitetura.

## Segurança

- **Preview obrigatório.** O tipo `ConfirmToken` só é construído por
  `Preview::confirm()`, então não existe caminho de código que execute uma
  transação sem ter passado por um preview aprovado.
- **Blocklist de essenciais.** `dnf5`, `glibc`, `systemd`, kernel, `sudo`,
  `rpm`, `polkit`, `grub2`, `dracut` e afins não podem ser marcados nem
  removidos: `dnfm remove` recusa com código 2 antes de qualquer preview e a
  TUI recusa o preview do protegido com o motivo no status.
- **O resolvedor é a rede final.** Se o DNF reportar qualquer problema de
  resolução, a execução é recusada — cobre dependências que a lista estática
  não conhece.
- **Marcações em arquivo.** `~/.local/state/dnfm/marks.json` e
  `~/.local/state/dnfm/categories.json`, gravação atômica; arquivo corrompido
  vira `*.json.corrupt` em vez de sumir.

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
cargo test --workspace            # 173 testes, sem tocar no sistema
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

## CI e release

- `.github/workflows/ci.yml` roda `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings` e `cargo test --workspace` em todo push e pull request.
  O toolchain é fixado em `1.99.1` para que os gates não mudem sozinhos.
- `.github/workflows/release.yml` dispara com uma tag `vX.Y.Z`. Ele reaproveita o job de
  CI, confere se a tag bate com a versão do workspace, compila dentro do Fedora,
  ajusta `packaging/dnfm.spec` com `packaging/bump_spec.py`, constrói o RPM e publica
  uma release com o `.rpm` e o tarball do binário.

```bash
git tag v0.3.0
git push origin v0.3.0
```

Para uma versão nova, suba antes o `version` de `[workspace.package]` no `Cargo.toml`
e o cabeçalho do `packaging/dnfm.1`; o spec é ajustado pela tag.

## Licença

MIT.
