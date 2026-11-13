//! Leitura de dicionários de atributos (`a{sv}` do D-Bus, objetos JSON nas
//! fixtures) por uma interface comum.
//!
//! Assim o mapeamento campo-a-campo é exercitado nos testes com payloads reais
//! capturados do daemon, sem precisar de uma conexão D-Bus.

/// Acesso tipado aos atributos de um pacote/advisory/transação.
pub trait AttrReader {
    fn get_str(&self, key: &str) -> Option<String>;
    fn get_u64(&self, key: &str) -> Option<u64>;
    fn get_i64(&self, key: &str) -> Option<i64>;
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn get_str_list(&self, key: &str) -> Option<Vec<String>>;

    fn str_or(&self, key: &str, default: &str) -> String {
        self.get_str(key).unwrap_or_else(|| default.to_string())
    }

    /// Texto vindo de string ou de número — o daemon usa os dois para IDs
    /// (`advisoryid` chega como inteiro).
    fn get_text(&self, key: &str) -> Option<String> {
        self.get_str(key)
            .or_else(|| self.get_i64(key).map(|v| v.to_string()))
    }
}

/// Implementação para os dicionários devolvidos pelo zbus.
#[cfg(feature = "dbus")]
impl AttrReader for std::collections::HashMap<String, zbus::OwnedVariant> {
    fn get_str(&self, key: &str) -> Option<String> {
        let value = self.get(key)?;
        let value: &zbus::Variant<'_> = value;
        <&str>::try_from(value).ok().map(str::to_string)
    }

    fn get_u64(&self, key: &str) -> Option<u64> {
        let value = self.get(key)?;
        let value: &zbus::Variant<'_> = value;
        u64::try_from(value)
            .ok()
            .or_else(|| u32::try_from(value).ok().map(u64::from))
            .or_else(|| {
                i64::try_from(value)
                    .ok()
                    .and_then(|v| u64::try_from(v).ok())
            })
            .or_else(|| {
                i32::try_from(value)
                    .ok()
                    .and_then(|v| u64::try_from(v).ok())
            })
    }

    fn get_i64(&self, key: &str) -> Option<i64> {
        let value = self.get(key)?;
        let value: &zbus::Variant<'_> = value;
        i64::try_from(value)
            .ok()
            .or_else(|| i32::try_from(value).ok().map(i64::from))
            .or_else(|| {
                u64::try_from(value)
                    .ok()
                    .and_then(|v| i64::try_from(v).ok())
            })
            .or_else(|| u32::try_from(value).ok().map(i64::from))
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        let value = self.get(key)?;
        let value: &zbus::Variant<'_> = value;
        bool::try_from(value).ok()
    }

    fn get_str_list(&self, key: &str) -> Option<Vec<String>> {
        let value = self.get(key)?;
        let value: &zbus::Variant<'_> = value;
        Vec::<String>::try_from(value.clone()).ok()
    }
}

/// Implementação para objetos JSON (usada nas fixtures de teste).
impl AttrReader for serde_json::Map<String, serde_json::Value> {
    fn get_str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str()).map(str::to_string)
    }

    fn get_u64(&self, key: &str) -> Option<u64> {
        let value = self.get(key)?;
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|v| u64::try_from(v).ok()))
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    }

    fn get_i64(&self, key: &str) -> Option<i64> {
        let value = self.get(key)?;
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    fn get_str_list(&self, key: &str) -> Option<Vec<String>> {
        self.get(key).and_then(|v| v.as_array()).map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
    }
}
