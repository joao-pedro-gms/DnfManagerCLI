# dnfm — Plano de desenvolvimento

TUI + CLI em Rust para gerenciar pacotes DNF no Fedora, com foco em **marcar
pacotes para remoção em lote**, ver e aplicar atualizações, e inspecionar
histórico/repos/órfãos.

Plano derivado de medições feitas neste sistema (Fedora 45, dnf5 5.4.7,
8.008 pacotes instalados, 48 atualizáveis), não de suposições.

---

## 1. Decisões fechadas

| Tema | Decisão | Por quê |
|---|---|---|
| Linguagem / TUI | Rust 1.99 + `ratatui` 0.30 + `crossterm` | Mouse completo (capture + hit-testing), multi-painel, sem GC |
| Backend | **100% D-Bus** no `dnf5daemon-server` via `zbus` 6.3 | `libdnf5` é C++ sem C ABI, `libdnf5-devel` ausente, bindings Rust inexistentes; o daemon já roda o libdnf5 in-process |
| Autenticação | Polkit, disparado pelo próprio daemon em `do_transaction` | Sem `sudo`, sem setuid; agente KDE já ativo |
| Carregamento | Bulk load único + filtro/ordenação em memória | 8.008 pacotes em 0,19–0,62 s; busca por padrão em 3 ms |
| Marcações | Estado em XDG, auto-save | Sobrevive a reinício; base para "limpar depois" |
| Proteção | Blocklist embutida + preview obrigatório + confirmação | Evitar auto-destruição do sistema |
| Testes | `trait DnfBackend` + fake em memória; integração real `#[ignore]` | Feedback em milissegundos, sem tocar no sistema |
| Distribuição | Spec RPM desde o início | Ferramenta nativa Fedora; `cargo install` fica como fallback |
| CLI | Espelho dos essenciais com `--json` | Automação/scripting |
| Offline updates | Fora do MVP (fase 2) | API `Offline` já existe no daemon |

---

## 2. Fatos medidos que sustentam o desenho

Comandos e tempos reais (sessão D-Bus, `org.rpm.dnf.v0`):

| Operação | Resultado | Tempo |
|---|---|---|
| `open_session` (só sistema) | ok | 6 ms |
| `open_session` (com repos disponíveis, cache quente) | ok | 10 ms |
| `Rpm.list scope=installed` (6 atributos) | 8.008 pacotes | 0,19–0,62 s |
| `Rpm.list scope=installed patterns=["fire*"]` | 5 pacotes | 3 ms |
| `Rpm.list scope=upgradable` | 48 pacotes | 0,94 s (carrega metadata) |
| `Advisory.list` | 14 advisories | 50 ms |
| `Rpm.remove(["dosbox-staging"])` | goal montado | ~0 ms |
| `Goal.resolve` | 5 itens, `result=0` | 0,33–1,35 s |
| `Goal.resolve` removendo `glibc` | 0 itens, `result=2` + problemas | 0,33 s |

Consequências diretas:

- **Não precisa de paginação nem cache em disco.** Um fetch no boot e filtro local.
- **`resolve` custa 0,3–1,4 s** → o preview tem que ser assíncrono com spinner, nunca bloqueando o render loop.
- **O daemon ignora `limit`** em `Rpm.list`: sempre devolve o conjunto inteiro. Payload grande é o único risco real (ver §10).

### Estrutura real de `transaction_items`

`Goal.resolve` devolve `a(sssa{sv}a{sv})`, e cada item é
`(tipo, ação, motivo, extras, atributos)`:

```
('Package', 'Remove', 'User',            {}, {name: 'dosbox-staging', install_size: 16417176, ...})
('Package', 'Remove', 'Dependency',      {}, {name: 'SDL2_net', ...})
('Package', 'Remove', 'Weak Dependency', {}, {name: 'iir1', ...})
('Package', 'Remove', 'Clean',           {}, {name: 'libslirp', ...})
```

Isso é o preview de dependências **de graça**: `motivo == User` é o que o
usuário marcou; o resto foi arrastado. `install_size` de cada item permite
somar o espaço liberado sem consulta extra.

### Assinaturas que NÃO seguem o padrão `a{sv}`

Armadilha verificada em teste — chamar com `a{sv}` retorna `InvalidArgs`:

- `Goal.resolve(a{sv})` — aceita `{}`.
- `Goal.get_transaction_problems_string()` / `get_transaction_problems()` — **sem argumentos**.
- `Goal.reset()` / `Goal.cancel()` — **sem argumentos**.
- `History.list({since: i64, limit: i64})` — `limit` em `i64`, não `u`.

### Superfície completa disponível

`SessionManager.open_session/close_session` · `Rpm.list/list_fd/install/upgrade/remove/distro_sync/downgrade/reinstall/system_upgrade` · `Goal.resolve/do_transaction/cancel/reset/get_transaction_problems*` · `Rpm.Repo.list/enable/disable/confirm_key` · `Advisory.list` (`advisory_attrs`, `availability`, `severities`, `reference_cves`, `with_cve`) · `History.list/recent_changes` · `Offline.*` · `Base.read_all_repos/clean/reset` · `comps.Group.list` · sinais de download e transação.

---

## 3. Arquitetura

```
┌─────────────────────────┐   comandos    ┌──────────────────────────┐
│ dnfm-tui (ratatui)      │ ────────────► │ dnfm-core                │
│  AppState + reducer     │ ◄──────────── │  trait DnfBackend        │
│  crossterm Events       │   snapshots   │  model / marks / safety  │
└─────────────────────────┘  + progresso  └───────────┬──────────────┘
┌─────────────────────────┐                           │
│ dnfm-cli (clap)         │ ──────────────────────────┤
└─────────────────────────┘                           │ zbus 6.3
                                          ┌───────────▼──────────────┐
                                          │ dnf5daemon-server        │
                                          │ org.rpm.dnf.v0 (polkit)  │
                                          └──────────────────────────┘
```

### Workspace

```
Cargo.toml                    # [workspace] members = ["crates/*"]
crates/dnfm-core/
  src/lib.rs
  src/model.rs                # Package, Action, RemovalReason, TransactionItem, Preview, Problem
  src/backend.rs              # trait DnfBackend + BackendError
  src/backend/dbus.rs         # proxies zbus + sessão + goal
  src/backend/fake.rs         # cenários em memória
  src/marks.rs                # MarkSet + XDG atômico + lock
  src/safety.rs               # blocklist
  src/query.rs                # filtro, ordenação, busca fuzzy
crates/dnfm-tui/
  src/lib.rs                  # run() -> Result
  src/app.rs                  # AppState, Action, update()
  src/keys.rs                 # teclado + mouse -> Action
  src/ui.rs                   # layout geral
  src/widgets/{table,detail,markbar,preview,progress,updates,help}.rs
crates/dnfm-cli/
  src/main.rs                 # clap + dispatch
  src/cmd/{list,updates,marks,remove,history}.rs
  src/output.rs               # tabela vs --json
crates/dnfm-core/tests/fixtures/   # payloads reais capturados do daemon
packaging/dnfm.spec
docs/plan.md
```

Regra do projeto: nenhum arquivo acima de 500 linhas — daí a divisão por widget.

### Núcleo

```rust
#[async_trait]
pub trait DnfBackend: Send + Sync {
    async fn installed(&self) -> Result<Vec<Package>, BackendError>;
    async fn upgradable(&self) -> Result<Vec<Update>, BackendError>;
    async fn advisories(&self, only_updates: bool) -> Result<Vec<Advisory>, BackendError>;
    async fn repos(&self) -> Result<Vec<Repo>, BackendError>;
    async fn history(&self, limit: i64) -> Result<Vec<HistoryEntry>, BackendError>;

    /// Substitui o goal pendente e resolve. `reset` sempre antes.
    async fn preview_remove(&self, specs: &[String]) -> Result<Preview, BackendError>;
    async fn preview_upgrade(&self, nevras: &[String]) -> Result<Preview, BackendError>;
    /// Só executa se houver preview com result < 2 e confirmação compatível.
    async fn execute(&self, confirm: ConfirmToken) -> Result<Outcome, BackendError>;

    fn progress(&self) -> broadcast::Receiver<ProgressEvent>;
}
```

`ConfirmToken` é construtor-privado e só nasce de `Preview::confirm()` — torna
impossível executar sem preview. É a proteção estrutural, não um `if`.

### Concorrência

Tokio `current_thread` no main loop; `tokio::select!` sobre (a) `crossterm`
`EventStream`, (b) canal de comandos, (c) `broadcast` de progresso. Uma única
sessão D-Bus (o `Goal` é estado único do daemon) → `preview` e `execute`
serializados por `Mutex` no backend.

---

## 4. Fluxos

### 4.1 Boot

1. `open_session { load_available_repos: false }` → 6 ms.
2. `Rpm.list scope=installed` com `name, evr, arch, repo_id, install_size, install_time, summary, reason` → pinta a tabela.
3. Em background: `Base.read_all_repos` + `Rpm.list scope=upgradable` + `Advisory.list { availability: "updates", advisory_attrs: [advisoryid,type,severity,title,reference_cves], with_cve: true }` → atualiza contadores do header.
4. Daemon indisponível → tela de erro com `systemctl start dnf5daemon-server` e modo read-only via `rpm -qa` (fallback opcional, fase 2).

### 4.2 Marcar → preview → executar (núcleo do produto)

1. `Space` marca/desmarca; `A` marca todos os visíveis; `U` limpa. Cada mudança é persistida em `marks.json` (debounce 300 ms).
2. `p` → `Goal.reset()` → `Rpm.remove(specs)` → `Goal.resolve({})`.
3. `result == 2` → tela de problemas com o texto de `get_transaction_problems_string()`; botão "aplicar" fica desabilitado.
4. `result < 2` → tela de preview agrupada por `RemovalReason`:
   - **Você marcou (N)** — `User`
   - **Dependências que saem junto (M)** — `Dependency` / `Weak Dependency`
   - **Limpeza (K)** — `Clean`
   - Total de espaço liberado = Σ `install_size`.
5. Confirmação: `y` para ≤ 20 pacotes; acima disso, digitar o número de pacotes.
6. `Goal.do_transaction { interactive: true }` → polkit pede senha → sinais alimentam a tela de progresso (`transaction_action_start/progress/stop`, scriptlets, `transaction_after_complete`).
7. Sucesso → remove os marcados aplicados, re-carrega instalados + `History.recent_changes`.
8. Falha/negado → mantém os marcados, mostra o erro do daemon.

### 4.3 Atualizações

Aba dedicada com colunas `severidade`, `advisory`, `CVE`, `atual → nova`, `download_size`.
Seleção por linha, por severidade (`!` filtra critical/important) e "selecionar todos os de segurança".
Aplicar usa `Rpm.upgrade(nevras)` + mesmo pipeline de preview/execute — o preview mostra o que entra como dependência.

### 4.4 Extras

- **Histórico**: `History.list { since, limit }` + `recent_changes` para o diff de uma transação.
- **Repos**: `Rpm.Repo.list`, `enable/disable` (exigem polkit), exibindo contagem de pacotes por repo.
- **Órfãos**: o daemon não expõe "leaves". Calcular localmente: pedir `requires` de todos os instalados (`Rpm.list` com `package_attrs=["requires"]`), montar grafo reverso e listar pacotes cujo `reason == User` e nenhum outro pacote instalado os requer. Cache em memória por sessão.

---

## 5. Segurança

Blocklist por regex sobre o nome (case-insensitive), sempre exibida como
"protegido" e nunca marcável:

```
^(dnf5?|libdnf5.*|dnf5daemon.*|rpm|rpm-libs|rpm-plugin.*)$
^(glibc|glibc-.*|bash|coreutils|coreutils-common|filesystem|setup)$
^(systemd|systemd-.*|udev.*|dbus|dbus-.*|polkit|polkit-.*)$
^(kernel|kernel-core|kernel-modules.*|kernel-.*)$
^(grub2-.*|shim.*|efibootmgr|dracut|dracut-.*)$
^(sudo|sudo-.*|openssh-server)$
^(selinux-policy.*|policycoreutils.*|firewalld.*)$
^(fedora-release.*|fedora-repos.*|fedora-gpg-keys)$
^python3(-libs|-dnf|-libdnf5)?$
```

Dinâmicas:
- kernel em execução (`kernel-$(uname -r)`) é protegido.
- `result == 2` do solver **sempre** bloqueia a execução — a rede de segurança real, porque o solver conhece requisitos que a blocklist não.
- nunca aplicar sem preview; nunca `--yes` implícito (o CLI exige `--yes` explícito e recusa se houver problemas).
- escrita de `marks.json` atômica (tmp + `fsync` + `rename`) com `flock`.

---

## 6. TUI

```
┌─ dnfm · Fedora 45 · 8.008 instalados · 48 atualizações · 3 marcados ────────┐
│ [Instalados] [Atualizações] [Histórico] [Repos]              busca: fire*  │
├─────────────────────────────────────────────────────────────────────────────┤
│  ☑  nome                 versão              repo    tamanho  instalado    │
│  ☐  firefox              160.0-1.fc45        fedora  312 MB   12/11       │
│▸ ☑  dosbox-staging       0.82.2-6.fc45       fedora  16 MB    03/10       │
├─────────────────────────────────────────────────────────────────────────────┤
│ dosbox-staging — DOSBox Staging is a modern continuation of DOSBox…          │
│ URL: https://www.dosbox-staging.org   Licença: GPL-2.0-or-later             │
├─────────────────────────────────────────────────────────────────────────────┤
│ ␣ marcar  p preview  / buscar  f filtros  s ordenar  ? ajuda  q sair        │
└─────────────────────────────────────────────────────────────────────────────┘
```

Teclado: `j/k/↑/↓`, `PgUp/PgDn`, `g/G`, `Space` marcar, `A` todos os visíveis,
`U` limpar, `/` busca fuzzy (3 ms para 8k em memória), `f` filtros (repo, motivo,
faixa de tamanho, data de instalação), `s` cicla ordenação, `Tab` painéis,
`Enter` detalhes, `p` preview, `!` só security updates, `?` ajuda, `q` sair
(avisa se houver marcas não aplicadas).

Mouse: clique seleciona, duplo-clique marca, scroll rola a tabela, clique no
header ordena, botões dos modais clicáveis. `EnableMouseCapture` + guarda de
restauração no panic hook.

---

## 7. CLI

```
dnfm                                  # TUI (default)
dnfm tui
dnfm list    [--scope installed|upgradable] [--repo ID]... [--pattern P]...
             [--sort name|size|date|repo] [--limit N] [--json]
dnfm updates [--advisory] [--severity critical|important|moderate|low] [--json]
dnfm marks   [--json] | add PKG... | rm PKG... | clear
dnfm remove  [PKG...] [--from-marks] [--dry-run] [--yes] [--json]
dnfm history [--limit N] [--json]
```

`remove` sem `--yes` imprime o preview (mesma tabela agrupada por motivo) e sai
com código 2 se houver problemas. Códigos: `0` ok, `1` erro, `2` problemas de
resolução, `3` autenticação negada, `4` daemon indisponível. `--json` emite
schema estável (`schema_version: 1`).

---

## 8. Testes

- **Fixtures reais**: capturar payloads `aa{sv}` do daemon em JSON e testar o
  parser contra eles (inclui o caso `reason` + campos ausentes).
- **Fake backend**: cenários `ok`, `blocked` (`result=2`), `polkit_denied`,
  `scriptlet_error`, `network_fail`, `daemon_down`.
- **Core**: filtros/ordenação (ordem total, sem duplicata), blocklist (casos
  positivos e negativos), `marks.json` (migração de versão, escrita atômica,
  arquivo corrompido → backup + estado limpo).
- **TUI**: `ratatui::backend::TestBackend` + snapshots de buffer para 80×24 e
  200×60; simulação de eventos de mouse.
- **Integração** (`#[ignore]`, `DNFM_REAL=1`): abre sessão real, lista, resolve
  um preview — **nunca** executa transação.
- **Contrato**: teste que falha ruidosamente se algum método/sinal esperado
  sumir da API do daemon (protege contra mudanças entre releases do Fedora).
- `cargo fmt --check`, `cargo clippy -- -D warnings` no CI.

---

## 9. Marcos

| Marco | Entrega | Critério de aceite |
|---|---|---|
| **M0** (~0,5 d) | Workspace, CI, spike zbus (open_session + list + resolve), esqueleto do spec | `cargo test` verde; spike imprime 8.008 pacotes; `dnfm --version` |
| **M1** (~2–3 d) | Read path: tabela, busca, filtros, ordenação, detalhe, mouse | Abre em <1,5 s com 8k; busca <50 ms; resize sem artefato; 100% teclado e mouse |
| **M2** (~3–4 d) | Marcações + preview + problemas + execução + progresso + blocklist | Preview agrupa por motivo; `glibc` bloqueado; transação real de 1 pacote concluída com polkit; marcas sobrevivem a restart |
| **M3** (~2 d) | Atualizações + advisories/severidade/CVE + aplicar seleção | 48 upgradables em <2 s; filtro por severidade; aplica só o selecionado |
| **M4** (~2–3 d) | Histórico, repos (enable/disable), órfãos | Órfãos de 8k em <1 s; disable de repo pede polkit |
| **M5** (~1–2 d) | RPM spec, man page, completions, COPR | `dnf install ./dnfm.rpm` funciona; `man dnfm` e completions instalados |

Total estimado: **11–14 dias** de trabalho efetivo.

---

## 10. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| `Rpm.list` ignora `limit` e devolve 8k em uma resposta única | `LimitsExceeded` em máquinas maiores | Fallback para `Rpm.list_fd` (streaming) se o erro aparecer; chunking por prefixo como último recurso |
| `Goal` é estado único da sessão | Preview concorrente corrompe o plano | `Mutex` no backend + `reset()` antes de todo `resolve` |
| `resolve` leva 0,3–1,4 s | UI travada | Sempre em task separada, com spinner e cancelamento |
| Daemon parado (`is-enabled: static`) | App não abre | Erro instrutivo + `systemctl start dnf5daemon-server`; detecção antes do primeiro call |
| Sem agente polkit (TTY puro) | Execução falha silenciosa | Detectar `polkit-kde`/`polkit-gnome`; mensagem com alternativa `pkexec dnf5` |
| Transação concorrente (dnf5 no terminal) | Lock do rpmdb | Surface do erro do daemon com instrução de retry |
| API v0 muda entre releases do Fedora | Quebra silenciosa | Teste de contrato no CI + erro explícito de versão |
| Remoção de kernel em uso | Sistema não boota | Proteção por `uname -r` + blocklist de `grub2`/`shim`/`dracut` |
| Mouse capture vaza em panic | Terminal inutilizável | Guarda `Drop` + panic hook restaurando o terminal |

### Explicitamente fora de escopo

Offline updates (`Offline.*`, fase 2), instalar pacotes novos, downgrade/distro-sync,
flatpak/snap, gerenciar conteúdo de repositórios (só enable/disable), i18n da
interface (só pt-BR e en no início).

---

## 11. Estado da implementação

Todos os marcos M0–M5 foram entregues, com três desvios conscientes.

| Item | Estado |
|---|---|
| Workspace (core/tui/cli), CI local (`fmt`, `clippy -D warnings`, testes) | ✅ |
| Leitura via D-Bus: 8.008 instalados, 48 atualizações, busca/ordenação em memória | ✅ |
| Marcações em XDG, escrita atômica, arquivo corrompido preservado | ✅ |
| Preview por motivo (`User` vs dependências), problemas do solver, `ConfirmToken` | ✅ |
| Execução com polkit e progresso por sinais | ✅ |
| Blocklist de essenciais + bloqueio por `result == 2` | ✅ |
| Aba de atualizações com severidade/advisory, seleção e aplicação | ✅ |
| Histórico, repos, órfãos (grafo reverso de `requires` em memória) | ✅ |
| CLI com `--json` estável (`schema_version: 1`), códigos de saída 0–4 | ✅ |
| man page, completions (bash/zsh/fish), spec RPM e RPM construído | ✅ |
| Testes: 46 (26 core, 14 TUI, 6 de render) + 6 de integração `#[ignore]` | ✅ |

Desvios em relação ao plano:

1. **`repos` é somente leitura na TUI.** Listar funciona; `enable`/`disable`
   existe na API do daemon mas não foi exposto — exige polkit e não estava no
   caminho crítico do MVP.
2. **Nenhuma transação real foi executada durante o desenvolvimento.** O
   caminho de execução é coberto por tipos (preview obrigatório), pelo backend
   fake, e pelo teste de integração que valida `resolve` contra o daemon real —
   mas a primeira remoção de verdade precisa ser feita por você.
3. **Advisory por pacote custa uma chamada por nome** (`contains_pkgs`, ~40 ms
   cada). Para 48 atualizações isso roda em background e não trava a UI; se o
   número crescer, vale paralelizar.

---

## 12. Extensão entregue: categorias, agrupamento e filtros

Pedido: agrupar pacotes por categorias e ter filtros e ordenações variados.

### O que a medição mostrou antes de decidir

| Fonte candidata | Realidade no sistema |
|---|---|
| Tag `Group` do RPM | 7.993 de 8.009 pacotes vêm como `Unspecified` → inútil |
| comps (`comps.Group.list`) | 7 grupos cobrindo 137 menções em 8.009 pacotes → insuficiente |
| Filtro `groups` no `Rpm.list` | ignorado silenciosamente pelo daemon (devolve tudo) |
| Prefixo do nome | 1.306 prefixos distintos, com cauda longa dominada por `texlive` (4.786) |

Conclusão: a taxonomia tinha que ser **derivada + editável**, não lida de
metadados que o Fedora não mantém mais.

### Modelo

Três camadas, da mais forte para a mais fraca:

1. **categoria própria** (`categories.json`, XDG) — atribuída por `t` na TUI ou
   `dnfm categories assign`; vence qualquer regra;
2. **categoria curada** — 60 apelidos de prefixo + 12 regras de palavra-chave,
   aplicadas ao nome; 30 categorias na instalação do autor, com "Outros" em 15%;
3. **família** — prefixo com apelidos, famílias < 3 pacotes em "Outros".

### Superfície entregue

- **TUI**: agrupamento (`o`) por categoria/família/repo/motivo, cabeçalhos
  recolhíveis (`espaço`), `a` marca o grupo inteiro, painel de filtros (`f`) com
  12 facetas, seletor multi-escolha com busca (`c`/`C`), atribuição de categoria
  (`t`/`T`), coluna de categoria na tabela e no painel de detalhes.
- **CLI**: `list --category/--family/--arch/--min-size/--max-size/
  --installed-since/--with-updates --sort {category,family,arch,…} --group-by
  {category,family,repo,reason}`, além de `dnfm categories [assign|rm|clear]`.
- **Busca/ordenação**: `Query` com contexto (marcas, taxonomia, atualizações)
  aplicando filtro E entre dimensões e OU dentro de categorias/famílias.

### Evidências

- 76 testes automatizados (38 core, 5 modelo, 8 lógica da TUI, 13 teclado,
  12 renderização) + 7 de integração `#[ignore]` (6 D-Bus + 1 render real).
- Teste de render com dados reais: 8.009 instalados → 30 categorias → 8.039
  linhas (pacotes + cabeçalhos), maiores grupos TeX Live (4.787), Outros
  (1.217), Bibliotecas (744); filtro por categoria casa exatamente a contagem
  do grupo.
- `cargo clippy --workspace --all-targets -- -D warnings` limpo.

### Limitações conhecidas

- O agrupamento é aplicado à aba de instalados; órfãos lista plano.
- Categorizar marcações em massa exige marcar primeiro (`t` usa os marcados, se
  houver; senão o pacote sob o cursor).
- Renomear categoria não existe: atribuir com o novo nome e remover a antiga.

---

---

## 13. Refinamento visual e animações da TUI

Pedido: TUI mais refinada, interativa, com animações e visualização clara dos
agrupamentos e filtros.

### Decisões

| Tema | Decisão | Por quê |
|---|---|---|
| Animação | motor próprio com `dt` e curvas de easing (`anim.rs`) | determinístico: os testes avançam o tempo à mão; nada depende da taxa de quadros |
| Laço | `frame_time()` adaptativo (16 ms animando, 200 ms parado) | animação suave sem gastar CPU à toa |
| Grupos | sidebar permanente com contagem e barra proporcional | responde "onde estão os pacotes" sem trocar de aba |
| Filtros | chips clicáveis no cabeçalho | remover um filtro sem abrir o painel de 12 facetas |
| Tema | três paletas com a mesma hierarquia semântica | trocar de paleta não quebra layout nem contraste |
| Modais | escala de 88%→100% com sombra deslocada | dar profundidade sem depender de transparência (terminal não tem alfa) |

### O que mudou na prática

- Layout em quatro zonas (cabeçalho 4 linhas, corpo com sidebar, detalhes,
  rodapé de 2 linhas com keycaps contextuais).
- Sidebar some abaixo de 92 colunas; o resto do layout se reacomoda.
- Tabela com zebra, hover, cabeçalhos de grupo com percentual e barra, números
  alinhados à direita e ícones `▣`/`▨`/`▢` para marcado/protegido/livre.
- Detalhes com dica quando os metadados ainda não foram carregados e destaque
  para categoria própria e proteções.
- `F2` troca a paleta e persiste em `ui.json`; `ctrl+b` alterna o sidebar.

### Evidências

- 100 testes (antes 76): 18 de renderização (sidebar, chips, colagem, tema,
  animação), 13 de teclado, 13 de unidade da TUI (animação, tema, sidebar,
  geometria de modal), 38 de core, 5 de modelo, 8 de lógica.
- Ferramenta de inspeção visual: `cargo run -p dnfm-tui --example snapshot`
 gera o buffer com cores e `scripts/snapshot_to_html.py` converte em HTML —
  usado para revisar o layout quadro a quadro durante o desenvolvimento.
- `clippy -D warnings` e `fmt` limpos.

### Limitações conhecidas

- Sem transparência real: o "dim" do fundo é obtido com sombra deslocada, não
  com alfa.
- Fechar um modal leva ~160 ms (bloqueia teclado nesse intervalo); abrir é
  imediato no primeiro quadro.
- O hover depende de o terminal reportar movimento do mouse.
- O resumo do sidebar conta sobre todos os instalados, não sobre o filtro atual.
