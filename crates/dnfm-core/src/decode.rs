//! Tradução dos dicionários do daemon para os tipos de domínio.

use crate::attrs::AttrReader;
use crate::model::{
    Action, Advisory, HistoryEntry, Package, PackageDetails, PlannedPackage, Preview, Reason, Repo,
    ResolveResult, Severity,
};

impl Package {
    /// Atributos pedidos em `package_attrs`.
    pub fn from_attrs<A: AttrReader>(attrs: &A) -> Self {
        let name = attrs.str_or("name", "");
        let arch = attrs.str_or("arch", "");
        let evr = attrs
            .get_str("evr")
            .or_else(|| {
                let version = attrs.get_str("version")?;
                let release = attrs.get_str("release").unwrap_or_default();
                Some(format!("{version}-{release}"))
            })
            .unwrap_or_default();
        let nevra = attrs.get_str("nevra").unwrap_or_else(|| {
            if name.is_empty() {
                String::new()
            } else {
                format!("{name}-{evr}.{arch}")
            }
        });
        Self {
            name,
            evr,
            arch,
            repo_id: attrs.str_or("repo_id", ""),
            from_repo_id: attrs.get_str("from_repo_id").filter(|s| !s.is_empty()),
            size: attrs
                .get_u64("install_size")
                .or_else(|| attrs.get_u64("download_size"))
                .unwrap_or(0),
            install_time: attrs.get_i64("install_time").unwrap_or(0),
            summary: attrs.str_or("summary", ""),
            reason: Reason::parse(&attrs.str_or("reason", "")),
            nevra,
            is_installed: attrs.get_bool("is_installed").unwrap_or(false),
        }
    }
}

impl PackageDetails {
    pub fn from_attrs<A: AttrReader>(attrs: &A) -> Self {
        Self {
            nevra: attrs.str_or("nevra", ""),
            url: attrs.str_or("url", ""),
            license: attrs.str_or("license", ""),
            description: attrs.str_or("description", ""),
            vendor: attrs.str_or("vendor", ""),
            group: attrs.str_or("group", ""),
        }
    }
}

impl PlannedPackage {
    /// `action`, `reason` e o dicionário de atributos de um item de transação.
    pub fn from_transaction<A: AttrReader>(action: &str, reason: &str, attrs: &A) -> Self {
        Self {
            action: Action::parse(action),
            reason: Reason::parse(reason),
            package: Package::from_attrs(attrs),
        }
    }
}

impl Preview {
    pub fn from_resolve(
        items: impl IntoIterator<Item = PlannedPackage>,
        problems: Vec<String>,
        result: u32,
    ) -> Self {
        Self {
            items: items.into_iter().collect(),
            problems,
            result: ResolveResult::parse(result),
        }
    }
}

impl Advisory {
    pub fn from_attrs<A: AttrReader>(attrs: &A) -> Self {
        Self {
            id: attrs.get_text("advisoryid").unwrap_or_default(),
            kind: attrs.str_or("type", ""),
            severity: Severity::parse(&attrs.str_or("severity", "")),
            title: attrs.str_or("title", ""),
        }
    }
}

impl Repo {
    pub fn from_attrs<A: AttrReader>(attrs: &A) -> Self {
        Self {
            id: attrs.str_or("id", ""),
            name: attrs.str_or("name", ""),
            enabled: attrs.get_bool("enabled").unwrap_or(false),
        }
    }
}

impl HistoryEntry {
    pub fn from_attrs<A: AttrReader>(attrs: &A) -> Self {
        fn names(value: Option<&serde_json::Value>) -> Vec<String> {
            value
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.get("name").and_then(|n| n.as_str()))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        }
        let raw = attrs.get_str("__raw");
        let parsed: Option<serde_json::Value> = raw.and_then(|r| serde_json::from_str(&r).ok());
        let get = |field: &str| parsed.as_ref().and_then(|v| v.get(field)).cloned();

        let id = attrs.get_i64("id").unwrap_or(0);
        let description = attrs.str_or("description", "");
        let status = attrs.str_or("status", "");
        let start = attrs.get_i64("start").unwrap_or(0);
        let end = attrs.get_i64("end").unwrap_or(0);
        let user_id = attrs.get_i64("user_id").unwrap_or(0);

        let installed = names(get("installed").as_ref());
        let removed = names(get("removed").as_ref());
        let upgraded = names(get("upgraded").as_ref());
        let downgraded = names(get("downgraded").as_ref());

        Self {
            id,
            description,
            status,
            start,
            end,
            user_id,
            installed,
            removed,
            upgraded,
            downgraded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_map(raw: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(raw)
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn pacote_instalado_das_fixtures_reais() {
        let fixture = include_str!("../tests/fixtures/installed.json");
        let first = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let first = first.as_array().unwrap()[0].as_object().unwrap();

        let pkg = Package::from_attrs(first);
        assert_eq!(pkg.name, "7zip");
        assert_eq!(pkg.evr, "26.04-1.fc45");
        assert_eq!(pkg.arch, "x86_64");
        assert_eq!(pkg.repo_id, "@System");
        assert_eq!(pkg.size, 3_516_415);
        assert_eq!(pkg.reason, Reason::Dependency);
        assert_eq!(pkg.summary, "A file archiver");
        assert!(!pkg.is_installed, "sem is_installed explicito na fixture");
    }

    #[test]
    fn pacote_atualizavel_das_fixtures_reais() {
        let fixture = include_str!("../tests/fixtures/upgradable.json");
        let items = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let first = items.as_array().unwrap()[0].as_object().unwrap();
        let pkg = Package::from_attrs(first);
        assert_eq!(pkg.name, "cockpit-bridge");
        assert_eq!(pkg.from_repo_id.as_deref(), Some("updates"));
    }

    #[test]
    fn preview_real_de_remocao() {
        let fixture = include_str!("../tests/fixtures/preview_remove.json");
        let doc = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let items = doc["items"].as_array().unwrap();

        let planned: Vec<PlannedPackage> = items
            .iter()
            .map(|item| {
                let tuple = item.as_array().unwrap();
                let attrs = tuple[4].as_object().unwrap();
                PlannedPackage::from_transaction(
                    tuple[1].as_str().unwrap(),
                    tuple[2].as_str().unwrap(),
                    attrs,
                )
            })
            .collect();

        let preview =
            Preview::from_resolve(planned, vec![], doc["result"].as_u64().unwrap() as u32);
        assert!(!preview.is_failed());
        assert!(preview
            .requested()
            .iter()
            .any(|i| i.package.name == "dosbox-staging"));
        assert!(preview
            .collateral()
            .iter()
            .any(|i| i.package.name == "SDL2_net"));
        assert!(preview.freed_bytes() > 0);
    }

    #[test]
    fn preview_bloqueado_das_fixtures() {
        let fixture = include_str!("../tests/fixtures/preview_blocked.json");
        let doc = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let problems: Vec<String> = doc["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let preview =
            Preview::from_resolve(Vec::new(), problems, doc["result"].as_u64().unwrap() as u32);
        assert!(preview.is_failed());
        assert!(!preview.problems.is_empty());
        assert!(preview.confirm().is_err());
    }

    #[test]
    fn advisory_das_fixtures() {
        let fixture = include_str!("../tests/fixtures/advisories.json");
        let items = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let first = items.as_array().unwrap()[0].as_object().unwrap();
        let advisory = Advisory::from_attrs(first);
        assert!(!advisory.id.is_empty());
        assert!(matches!(
            advisory.kind.as_str(),
            "security" | "bugfix" | "enhancement" | "newpackage"
        ));
    }

    #[test]
    fn repos_das_fixtures() {
        let fixture = include_str!("../tests/fixtures/repos.json");
        let items = serde_json::from_str::<serde_json::Value>(fixture).unwrap();
        let first = items.as_array().unwrap()[0].as_object().unwrap();
        let repo = Repo::from_attrs(first);
        assert!(!repo.id.is_empty());
        assert!(repo.enabled);
    }

    #[test]
    fn evr_e_derivado_quando_ausente() {
        let map =
            json_map(r#"{"name":"vlc","arch":"x86_64","version":"3.0.24","release":"3.fc45"}"#);
        let pkg = Package::from_attrs(&map);
        assert_eq!(pkg.evr, "3.0.24-3.fc45");
        assert_eq!(pkg.nevra, "vlc-3.0.24-3.fc45.x86_64");
    }
}
