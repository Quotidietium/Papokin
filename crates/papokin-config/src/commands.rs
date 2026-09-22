use std::collections::HashMap;

use papokin_util::PermissionLvl;
use serde::{Deserialize, Serialize};

/// 命令处理与执行的配置。
///
/// 控制命令的接受与记录方式，以及需要哪种权限
/// 非管理员玩家默认获得的权限等级。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct CommandsConfig {
    /// 是否接受来自控制台的命令。
    pub use_console: bool,
    /// 是否使用 rustyline 处理 TTY 输入。
    pub use_tty: bool,
    /// 玩家的命令是否记录到控制台。
    pub log_console: bool,
    /// 控制台和 RCON 命令输出是否广播给在线管理员。
    /// 对应原版的 `broadcast-console-to-ops` 服务器属性。
    pub broadcast_console_to_ops: bool,
    /// 所有不在 `ops` 文件中的玩家的 `op` 权限等级。
    pub default_op_level: PermissionLvl,
    /// 每条命令的设置，可单独关闭某条命令或更改
    /// 谁可以使用它们。
    ///
    /// 每个条目以其适用的命令命名（不含前导
    /// 斜杠），例如 `gamemode` 或 `tp`。你只需列出
    /// 你真正想修改的命令；未列出的内容都会保留其
    /// 正常行为。
    ///
    /// 示例：
    ///
    /// ```toml
    /// # Only server owners may change gamemodes
    /// [commands.overrides.gamemode]
    /// permission_level = 4
    ///
    /// # Turn the /tp command off completely
    /// [commands.overrides.tp]
    /// enabled = false
    /// ```
    pub overrides: HashMap<String, CommandOverride>,
}

impl Default for CommandsConfig {
    fn default() -> Self {
        Self {
            use_console: true,
            log_console: true,
            use_tty: true,
            broadcast_console_to_ops: true,
            default_op_level: PermissionLvl::Zero,
            overrides: HashMap::new(),
        }
    }
}

/// 单个命令的设置，允许服务器管理员将其关闭或更改
/// 谁可以运行它。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct CommandOverride {
    /// 此命令是否完全可用。设为 `false` 时，该命令
    /// 会被完全隐藏：它不会运行、不会出现在命令列表中，
    /// 也不会出现在 Tab 补全中。尝试使用它的玩家只会得到
    /// 正常的“未知命令”消息。设为 `true`（默认）即可
    /// 保持命令可用。
    pub enabled: bool,
    /// 允许谁使用此命令，以权限级别给出：
    ///
    /// - `0` = 任何人都可以使用
    /// - `2` = 普通操作员（大多数作弊类命令的常用等级）
    /// - `3` = 管理员（玩家管理、踢出、封禁等）
    /// - `4` = 仅服务器所有者（完整的服务器管理）
    ///
    /// 省略此项可保留命令的常规要求。
    pub permission_level: Option<PermissionLvl>,
}

impl Default for CommandOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            permission_level: None,
        }
    }
}
