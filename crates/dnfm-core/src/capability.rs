//! Normalização de capabilities de RPM para o grafo de dependências.
//!
//! `provides` e `requires` chegam do daemon em formatos diferentes para a mesma
//! capability: `libdrm(x86-64) = 2.4.124-1.fc45` de um lado, `libdrm` ou
//! `libdrm >= 2.4` do outro, além de rich deps como `(libnotify or libnotify4)`
//! e das entradas `rpmlib(...)`, que só descrevem o próprio rpm. Comparar as
//! string cruas casava menos da metade dos requires (46,6% medidos no daemon),
//! então a comparação é feita sobre o nome normalizado da capability.
//!
//! Além de capabilities, um require pode ser um caminho absoluto de arquivo
//! (`/usr/bin/ruby`, `/usr/bin/systemd-sysusers`): esses nunca casam com
//! provides de nome, então são resolvidos pelo mapa de donos de arquivo
//! construído do atributo `files` dos pacotes instalados.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::model::Reason;

/// Nome da capability: o token antes do primeiro espaço, parêntese ou operador
/// de comparação.
///
/// `None` quando a entrada não participa do grafo (`rpmlib(...)` ou vazia).
pub fn capability_name(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("rpmlib(") {
        return None;
    }
    let name: String = raw
        .chars()
        .take_while(|c| !c.is_whitespace() && !matches!(c, '(' | '=' | '<' | '>'))
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Nomes exigidos por uma entrada de `requires`, sem repetição.
///
/// Rich deps `(a or b)`, `(a if b)` e `(a if b else c)` viram alternativas:
/// basta um dos nomes para o pacote que o oferece deixar de ser órfão.
pub fn requirement_names(raw: &str) -> Vec<String> {
    let mut nomes: Vec<String> = Vec::new();
    for nome in alternatives(raw).into_iter().filter_map(capability_name) {
        if !nomes.contains(&nome) {
            nomes.push(nome);
        }
    }
    nomes
}

/// Quebra `(a or b)`, `(a if b)` e aninhamentos em alternativas simples.
fn alternatives(raw: &str) -> Vec<&str> {
    let raw = raw.trim();
    let Some(inner) = raw.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return vec![raw];
    };
    // `(libfoo) >= 1` não é rich dep: o parêntese não envolve a expressão toda.
    if !balanceado(inner) {
        return vec![raw];
    }
    let mut out = Vec::new();
    for parte in split_operadores(inner) {
        out.extend(alternatives(parte));
    }
    out
}

fn balanceado(inner: &str) -> bool {
    let mut profundeza = 0i32;
    for c in inner.chars() {
        match c {
            '(' => profundeza += 1,
            ')' => {
                profundeza -= 1;
                if profundeza < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    profundeza == 0
}

/// Corta a expressão nos conectivos `or`/`if` de nível zero.
fn split_operadores(inner: &str) -> Vec<&str> {
    let bytes = inner.as_bytes();
    let mut partes = Vec::new();
    let mut profundeza = 0i32;
    let mut inicio = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => profundeza += 1,
            b')' => profundeza -= 1,
            b' ' if profundeza == 0 => {
                let resto = &inner[i..];
                let pulo = if resto.starts_with(" or ") || resto.starts_with(" if ") {
                    " or ".len()
                } else if resto.starts_with(" else ") {
                    " else ".len()
                } else {
                    0
                };
                if pulo > 0 {
                    partes.push(inner[inicio..i].trim());
                    i += pulo;
                    inicio = i;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    partes.push(inner[inicio..].trim());
    partes
}

/// Índice capability normalizada -> chaves (`nome.arch`) que a oferecem.
#[derive(Debug, Default)]
struct ProviderIndex {
    providers: HashMap<String, HashSet<String>>,
}

impl ProviderIndex {
    fn add(&mut self, key: &str, provides: &[String]) {
        for raw in provides {
            if let Some(nome) = capability_name(raw) {
                self.providers
                    .entry(nome)
                    .or_default()
                    .insert(key.to_string());
            }
        }
    }
}

/// Mapa `caminho absoluto` -> chaves (`nome.arch`) dos pacotes que possuem o
/// arquivo, em ordem estável. Um caminho pode ter mais de um dono (arquivos
/// compartilhados entre pacotes), e por conservadorismo todos seguram os seus.
pub type FileOwners = HashMap<String, Vec<String>>;

/// Constrói o mapa de donos de arquivo do atributo `files` dos registros.
///
/// Serve para resolver requires que são caminhos absolutos; caminhos que não
/// são absolutos, vazios ou com `/` no fim (exceto a raiz) são descartados.
pub fn file_owners(records: &[InstalledRecord]) -> FileOwners {
    let mut owners: HashMap<String, BTreeSet<String>> = HashMap::new();
    for record in records {
        for caminho in &record.files {
            if let Some(chave) = normaliza_caminho(caminho) {
                owners.entry(chave).or_default().insert(record.key.clone());
            }
        }
    }
    owners
        .into_iter()
        .map(|(caminho, chaves)| (caminho, chaves.into_iter().collect()))
        .collect()
}

/// Caminho utilizável como chave: absoluto e sem `/` no fim (exceto a raiz).
fn normaliza_caminho(raw: &str) -> Option<String> {
    let cru = raw.trim();
    if cru.is_empty() || !cru.starts_with('/') {
        return None;
    }
    let mut cru = cru;
    while cru.len() > 1 && cru.ends_with('/') {
        cru = &cru[..cru.len() - 1];
    }
    Some(cru.to_string())
}

/// Um pacote instalado, reduzido ao que decide se ele é órfão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRecord {
    /// `nome.arch`, a mesma chave de [`crate::model::Package::key`].
    pub key: String,
    pub reason: Reason,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    /// Caminhos absolutos dos arquivos do pacote; vazio quando a consulta não
    /// pediu o atributo `files`, e aí requires de caminho ficam sem dono.
    pub files: Vec<String>,
}

/// Chaves que algum outro pacote instalado requer.
///
/// Require de capability (`libdrm`, rich deps) casa com o índice de provides;
/// require de caminho absoluto (`/usr/bin/ruby`) casa com o mapa de donos de
/// arquivo e também com provide explícito do caminho (`/usr/sbin/ipsec` é
/// provide virtual de `libreswan`, não arquivo dele). O dono fica requerido,
/// não quem exige.
/// Marca cada provedor como requerido, menos o próprio requisor.
fn segurar<'a>(
    donos: impl Iterator<Item = &'a String>,
    record: &InstalledRecord,
    required: &mut HashSet<String>,
) {
    for dono in donos {
        if dono != &record.key {
            required.insert(dono.clone());
        }
    }
}

pub fn required_keys(records: &[InstalledRecord]) -> HashSet<String> {
    let owners = file_owners(records);
    let mut index = ProviderIndex::default();
    for record in records {
        index.add(&record.key, &record.provides);
    }

    let mut required = HashSet::new();
    for record in records {
        for raw in &record.requires {
            if let Some(caminho) = normaliza_caminho(raw) {
                segurar(
                    owners.get(&caminho).into_iter().flatten(),
                    record,
                    &mut required,
                );
                // Dono virtual: provide explícito do caminho (`/usr/sbin/ipsec`).
                segurar(
                    index.providers.get(&caminho).into_iter().flatten(),
                    record,
                    &mut required,
                );
                continue;
            }
            for nome in requirement_names(raw) {
                segurar(
                    index.providers.get(&nome).into_iter().flatten(),
                    record,
                    &mut required,
                );
            }
        }
    }
    required
}

/// Só o que entrou como dependência vira sugestão de remoção: pacote pedido
/// pelo usuário ou puxado por um grupo de pacotes não é órfão.
pub fn is_orphan_reason(reason: Reason) -> bool {
    matches!(reason, Reason::Dependency | Reason::WeakDependency)
}

/// Chaves órfãs, sem repetição e em ordem estável.
pub fn orphan_keys(records: &[InstalledRecord]) -> Vec<String> {
    let required = required_keys(records);
    let mut keys: Vec<String> = records
        .iter()
        .filter(|r| is_orphan_reason(r.reason) && !required.contains(&r.key))
        .map(|r| r.key.clone())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}
