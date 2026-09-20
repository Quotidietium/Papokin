//! Server-level permission declarations (`permissions.toml`).
//!
//! Server owners can declare permission nodes ahead of plugin registration —
//! the equivalent of Bukkit's `permissions.yml`. Declared defaults override
//! what a plugin registers later, and player attachments grant or deny nodes
//! explicitly per UUID.
//!
//! # Format
//!
//! ```toml
//! [permissions."myplugin:command.home"]
//! description = "Allow using /home"
//! default = "op:1"          # "true", "false", "op" or "op:<0-4>"
//! children = { "myplugin:command.home.others" = true }
//!
//! [players."<uuid>"]
//! granted = ["myplugin:command.home"]
//! denied = ["myplugin:command.home.others"]
//! ```

use pumpkin_util::permission::{Permission, PermissionDefault, PermissionLvl, PermissionManager};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PermissionsFile {
    #[serde(default)]
    permissions: BTreeMap<String, PermissionDeclaration>,
    #[serde(default)]
    players: BTreeMap<String, PlayerDeclaration>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PermissionDeclaration {
    #[serde(default)]
    description: String,
    #[serde(default)]
    default: String,
    #[serde(default)]
    children: BTreeMap<String, bool>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PlayerDeclaration {
    #[serde(default)]
    granted: Vec<String>,
    #[serde(default)]
    denied: Vec<String>,
}

fn parse_default(raw: &str) -> Result<PermissionDefault, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "false" | "deny" => Ok(PermissionDefault::Deny),
        "true" | "allow" => Ok(PermissionDefault::Allow),
        "op" => Ok(PermissionDefault::Op(PermissionLvl::Two)),
        level if level.starts_with("op:") => {
            let level = level["op:".len()..]
                .parse::<u8>()
                .map_err(|_| format!("invalid op level in `{level}`"))?;
            match level {
                0..=4 => Ok(PermissionDefault::Op(match level {
                    0 => PermissionLvl::Zero,
                    1 => PermissionLvl::One,
                    2 => PermissionLvl::Two,
                    3 => PermissionLvl::Three,
                    _ => PermissionLvl::Four,
                })),
                _ => Err(format!("op level {level} out of range 0-4")),
            }
        }
        other => Err(format!(
            "unknown default `{other}` (expected true, false, op or op:<0-4>)"
        )),
    }
}

/// Loads `permissions.toml` into the [`PermissionManager`], if it exists.
///
/// Returns a summary of applied entries on success.
pub fn load_permissions_file(manager: &PermissionManager, path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(String::new());
    }
    let file: PermissionsFile =
        toml::from_str(&content).map_err(|e| format!("invalid TOML: {e}"))?;

    let mut applied = 0usize;
    for (node, declaration) in &file.permissions {
        let default = parse_default(&declaration.default)?;
        if manager.has_registered_permission(node) {
            let _ = manager.set_default(node, default);
        } else {
            let mut permission = Permission::new(node, &declaration.description, default);
            for (child, value) in &declaration.children {
                permission.add_child(child, *value);
            }
            let _ = manager.register_permission(permission);
        }
        applied += 1;
    }

    let mut players = 0usize;
    for (uuid, declaration) in &file.players {
        let Ok(uuid) = uuid::Uuid::parse_str(uuid) else {
            return Err(format!("invalid player uuid `{uuid}`"));
        };
        for node in &declaration.granted {
            manager.set_permission(uuid, node.clone(), true);
        }
        for node in &declaration.denied {
            manager.set_permission(uuid, node.clone(), false);
        }
        players += 1;
    }

    Ok(format!("{applied} permissions, {players} player entries"))
}

#[cfg(test)]
mod tests {
    use super::parse_default;
    use pumpkin_util::permission::{PermissionDefault, PermissionLvl};

    #[test]
    fn parses_defaults() {
        assert_eq!(parse_default("true").unwrap(), PermissionDefault::Allow);
        assert_eq!(parse_default("false").unwrap(), PermissionDefault::Deny);
        assert_eq!(
            parse_default("op").unwrap(),
            PermissionDefault::Op(PermissionLvl::Two)
        );
        assert_eq!(
            parse_default("op:4").unwrap(),
            PermissionDefault::Op(PermissionLvl::Four)
        );
        assert!(parse_default("banana").is_err());
        assert!(parse_default("op:9").is_err());
    }
}
