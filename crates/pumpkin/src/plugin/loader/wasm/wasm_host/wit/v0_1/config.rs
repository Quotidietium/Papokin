use crate::plugin::loader::wasm::wasm_host::{state::PluginHostState, wit::v0_1::pumpkin};
use std::path::PathBuf;
use toml::Value;

fn config_path(plugin_name: &str) -> PathBuf {
    std::path::Path::new("plugins")
        .join("data")
        .join(plugin_name)
        .join("config.toml")
}

fn merge_values(defaults: Value, user: Value) -> Value {
    match (defaults, user) {
        (Value::Table(defaults), Value::Table(user)) => {
            let mut out = defaults;
            for (key, value) in user {
                let merged = match out.get(&key) {
                    Some(default) => merge_values(default.clone(), value),
                    None => value,
                };
                out.insert(key, merged);
            }
            Value::Table(out)
        }
        (_, user) => user,
    }
}

fn merge_toml(defaults: &str, existing: &str) -> Result<String, String> {
    if existing.trim().is_empty() {
        return Ok(defaults.to_string());
    }
    if defaults.trim().is_empty() {
        return Ok(existing.to_string());
    }
    let defaults: Value =
        toml::from_str(defaults).map_err(|e| format!("invalid default config: {e}"))?;
    let existing: Value =
        toml::from_str(existing).map_err(|e| format!("invalid config file: {e}"))?;
    let merged = merge_values(defaults, existing);
    match merged {
        table @ Value::Table(_) => {
            toml::to_string(&table).map_err(|e| format!("failed to serialize config: {e}"))
        }
        scalar => Ok(scalar.to_string()),
    }
}

#[allow(clippy::unused_async_trait_impl)]
impl pumpkin::plugin::config::Host for PluginHostState {
    async fn load_config(&mut self, defaults: String) -> wasmtime::Result<Result<String, String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("plugin name is not set".to_string()));
        };

        let path = config_path(&name);
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        let merged = match merge_toml(&defaults, &existing) {
            Ok(merged) => merged,
            Err(error) => return Ok(Err(error)),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| wasmtime::Error::msg(format!("failed to create config dir: {e}")))?;
        }
        std::fs::write(&path, &merged)
            .map_err(|e| wasmtime::Error::msg(format!("failed to write config: {e}")))?;
        Ok(Ok(merged))
    }

    async fn save_config(&mut self, content: String) -> wasmtime::Result<Result<(), String>> {
        let Some(name) = self.name.clone() else {
            return Ok(Err("plugin name is not set".to_string()));
        };

        if !content.trim().is_empty() && toml::from_str::<Value>(&content).is_err() {
            return Ok(Err("content is not valid TOML".to_string()));
        }

        let path = config_path(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| wasmtime::Error::msg(format!("failed to create config dir: {e}")))?;
        }
        std::fs::write(&path, content)
            .map_err(|e| wasmtime::Error::msg(format!("failed to write config: {e}")))?;
        Ok(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::merge_toml;

    #[test]
    fn merges_user_values_over_defaults() {
        let defaults = "a = 1\n[server]\nhost = \"localhost\"\nport = 8080\n";
        let existing = "[server]\nport = 9090\n";
        let merged = merge_toml(defaults, existing).unwrap();
        let value: toml::Value = toml::from_str(&merged).unwrap();
        assert_eq!(value["a"].as_integer(), Some(1));
        assert_eq!(value["server"]["host"].as_str(), Some("localhost"));
        assert_eq!(value["server"]["port"].as_integer(), Some(9090));
    }

    #[test]
    fn empty_existing_returns_defaults() {
        assert_eq!(merge_toml("a = 1\n", "").unwrap(), "a = 1\n");
    }

    #[test]
    fn empty_defaults_returns_existing() {
        assert_eq!(merge_toml("", "a = 1\n").unwrap(), "a = 1\n");
    }

    #[test]
    fn invalid_toml_is_rejected() {
        assert!(merge_toml("a = 1\n", "= = bad").is_err());
    }
}
