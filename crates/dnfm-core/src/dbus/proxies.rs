//! Proxies zbus das interfaces do `dnf5daemon`.
//!
//! O daemon usa snake_case nos nomes de métodos, então cada um precisa de
//! `#[zbus(name = ...)]` — o zbus converteria para PascalCase por padrão.

use std::collections::HashMap;

use zbus::{OwnedObjectPath, OwnedVariant as OwnedValue, Variant as Value};

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.SessionManager",
    default_service = "org.rpm.dnf.v0",
    default_path = "/org/rpm/dnf/v0"
)]
pub trait SessionManager {
    #[zbus(name = "open_session")]
    fn open_session(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(name = "close_session")]
    fn close_session(&self, session: &OwnedObjectPath) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.rpm.Rpm",
    default_service = "org.rpm.dnf.v0"
)]
pub trait Rpm {
    #[zbus(name = "list")]
    fn list(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
    #[zbus(name = "remove")]
    fn remove(
        &self,
        specs: &[String],
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<()>;
    #[zbus(name = "upgrade")]
    fn upgrade(
        &self,
        specs: &[String],
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    #[zbus(name = "transaction_before_begin")]
    fn transaction_before_begin(&self, session: OwnedObjectPath, total: u64) -> zbus::Result<()>;
    #[zbus(signal)]
    #[zbus(name = "transaction_action_start")]
    fn transaction_action_start(
        &self,
        session: OwnedObjectPath,
        nevra: String,
        action: u32,
        total: u64,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    #[zbus(name = "transaction_action_progress")]
    fn transaction_action_progress(
        &self,
        session: OwnedObjectPath,
        nevra: String,
        processed: u64,
        total: u64,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    #[zbus(name = "transaction_action_stop")]
    fn transaction_action_stop(
        &self,
        session: OwnedObjectPath,
        nevra: String,
        total: u64,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    #[zbus(name = "transaction_script_error")]
    fn transaction_script_error(
        &self,
        session: OwnedObjectPath,
        nevra: String,
        scriptlet_type: u32,
        return_code: u32,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    #[zbus(name = "transaction_after_complete")]
    fn transaction_after_complete(
        &self,
        session: OwnedObjectPath,
        success: bool,
    ) -> zbus::Result<()>;
}

#[zbus::proxy(interface = "org.rpm.dnf.v0.Goal", default_service = "org.rpm.dnf.v0")]
pub trait Goal {
    #[allow(clippy::type_complexity)]
    #[zbus(name = "resolve")]
    fn resolve(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<(
        Vec<(
            String,
            String,
            String,
            HashMap<String, OwnedValue>,
            HashMap<String, OwnedValue>,
        )>,
        u32,
    )>;
    #[zbus(name = "do_transaction")]
    fn do_transaction(&self, options: HashMap<String, Value<'static>>) -> zbus::Result<()>;
    #[zbus(name = "reset")]
    fn reset(&self) -> zbus::Result<()>;
    #[zbus(name = "get_transaction_problems_string")]
    fn get_transaction_problems_string(&self) -> zbus::Result<Vec<String>>;
}

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.Advisory",
    default_service = "org.rpm.dnf.v0"
)]
pub trait AdvisoryApi {
    #[zbus(name = "list")]
    fn list(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.rpm.Repo",
    default_service = "org.rpm.dnf.v0"
)]
pub trait RepoApi {
    #[zbus(name = "list")]
    fn list(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.History",
    default_service = "org.rpm.dnf.v0"
)]
pub trait HistoryApi {
    #[zbus(name = "list")]
    fn list(
        &self,
        options: HashMap<String, Value<'static>>,
    ) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}
