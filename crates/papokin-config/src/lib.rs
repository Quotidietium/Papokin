//! Pumpkin Minecraft 服务器的配置管理与序列化。
#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use fun::FunConfig;

use logging::LoggingConfig;
use papokin_util::world_seed::Seed;
use papokin_util::{Difficulty, GameMode, PermissionLvl, random};
use recipe::RecipeConfig;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use std::path::PathBuf;
use std::{fs, num::NonZero, path::Path};
use tracing::{debug, error, warn};

/// 趣味性与实验性配置选项。
pub mod fun;
/// 服务器日志配置选项。
pub mod logging;
/// 网络与协议配置选项。
pub mod networking;
/// 插件管理配置选项。
pub mod plugins;
/// 配方系统配置选项。
pub mod recipe;

/// 资源包配置选项。
pub mod resource_pack;

pub use chat::{AntiSpamConfig, ChatConfig};
pub use commands::{CommandOverride, CommandsConfig};
pub use networking::auth::AuthenticationConfig;
pub use networking::compression::CompressionConfig;
pub use networking::java::JavaConfig;
pub use networking::lan_broadcast::LANBroadcastConfig;
pub use networking::packet_limiter::PacketLimiterConfig;
pub use networking::rcon::RCONConfig;
pub use plugins::{PluginOverride, PluginsConfig};
pub use pvp::PVPConfig;
pub use server_links::ServerLinksConfig;
pub use telemetry::TelemetryConfig;

/// 遥测配置选项。
pub mod telemetry;

mod commands;

mod chat;
/// 区块加载与保存的配置选项。
pub mod chunk;
/// 光照引擎的配置选项。
pub mod lighting;
/// 操作员权限等级配置选项。
pub mod op;

mod advancement;
mod player_data;
mod pvp;
mod server_links;
/// 白名单配置选项。
pub mod whitelist;
/// 世界生成与维度配置选项。
pub mod world;

use advancement::AdvancementConfig;
use networking::NetworkingConfig;
use player_data::PlayerDataConfig;
use resource_pack::ResourcePackConfig;
use world::LevelConfig;

/// Pumpkin 服务器设置的根配置容器。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PapokinConfig {
    /// 核心基础配置设置。
    #[serde(flatten)]
    pub basic: BasicConfiguration,
    /// 高级与特定功能的配置设置。
    #[serde(flatten)]
    pub advanced: AdvancedConfiguration,
    /// 匿名遥测配置设置。
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

impl LoadConfiguration for PapokinConfig {
    fn get_path() -> &'static Path {
        Path::new("papokin.toml")
    }

    fn get_legacy_path() -> Option<&'static Path> {
        Some(Path::new("pumpkin.toml"))
    }

    fn validate(&self) {
        self.basic.validate();
        self.advanced.validate();
        self.telemetry.validate();

        let Some(min_vd) = NonZero::new(2) else {
            return;
        };
        let Some(max_vd) = NonZero::new(64) else {
            return;
        };

        // 校验 Java
        assert!(
            self.advanced.networking.java.keep_alive_time > 0,
            "Java 保活时间必须大于 0"
        );
        assert!(
            self.advanced.networking.java.view_distance >= min_vd,
            "Java 视距必须至少为 2"
        );
        assert!(
            self.advanced.networking.java.view_distance <= max_vd,
            "Java 视距必须小于 64"
        );
        if self.advanced.networking.java.online_mode {
            assert!(
                self.advanced.networking.java.encryption,
                "启用在线模式时必须同时启用加密"
            );
        }

        if self.basic.allow_chat_reports {
            assert!(
                self.advanced.networking.java.online_mode,
                "启用 allow_chat_reports 时必须启用 java.online_mode"
            );
        }
    }
}

/// 针对可选和特定功能服务器设置的高级配置。
///
/// 允许启用/禁用功能、自定义行为，以及
/// 调整性能或实验性选项。
///
/// `Important`：配置默认应与原版保持一致。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct AdvancedConfiguration {
    /// 与日志相关的配置，例如日志级别和输出行为。
    pub logging: LoggingConfig,
    /// 资源包配置，包括强制启用策略与资源包元数据。
    pub resource_pack: ResourcePackConfig,
    /// 基础配置之外与世界及世界等级相关的设置。
    pub world: LevelConfig,
    /// 网络相关功能，例如压缩、身份验证和 LAN 广播。
    pub networking: NetworkingConfig,
    /// 命令系统配置，包括可用性与权限。
    pub commands: CommandsConfig,
    /// 聊天相关功能，例如格式化、过滤和消息行为。
    pub chat: ChatConfig,
    /// 玩家对战（PvP）规则与机制。
    pub pvp: PVPConfig,
    /// 暴露给客户端的服务器链接配置。
    pub server_links: ServerLinksConfig,
    /// 玩家持久数据的处理与存储行为。
    pub player_data: PlayerDataConfig,
    /// 可选的趣味与实验性功能。
    pub fun: FunConfig,
    /// 配方相关配置。
    pub recipe: RecipeConfig,
    /// 插件相关配置。
    pub plugins: PluginsConfig,
    /// 进度配置
    pub advancement: AdvancementConfig,
}

/// 核心服务器设置的基础配置。
///
/// 涵盖版本支持、世界、网络、游戏规则和安全选项。
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct BasicConfiguration {
    /// 用于世界生成的种子。
    pub seed: Seed,
    /// 默认游戏难度。
    pub default_difficulty: Difficulty,
    /// 由 /op 命令分配的 OP 等级。
    pub op_permission_level: PermissionLvl,
    /// 是否启用下界维度。
    pub allow_nether: bool,
    /// 是否启用末地维度。
    pub allow_end: bool,
    /// 服务器是否处于极限模式。
    pub hardcore: bool,
    /// 服务器的每秒刻数（TPS）。
    pub tps: f32,
    /// 玩家的默认游戏模式。
    pub default_gamemode: GameMode,
    /// 如果服务器在玩家加入时强制应用游戏模式。
    pub force_gamemode: bool,
    /// 是否从日志中移除 IP。
    pub scrub_ips: bool,
    /// 是否使用服务器图标。
    pub use_favicon: bool,
    /// 可选服务器图标的路径。
    pub favicon_path: Option<String>,
    /// 默认的世界名称
    pub default_level_name: String,
    /// 聊天消息是否需要签名。
    pub allow_chat_reports: bool,
    /// 是否启用白名单。
    pub white_list: bool,
    /// 是否强制执行白名单。
    pub enforce_whitelist: bool,
    /// 此服务器是否接受来自其他服务器的转移连接。
    #[serde(alias = "accepts-transfers", alias = "accepts_transfers")]
    pub accepts_transfers: bool,
    /// 世界出生点周围出生保护区域的半径。
    /// 没有管理员权限的玩家不能在此半径内破坏或放置方块。
    /// 设为 0 可禁用出生点保护。
    pub spawn_protection: u32,
}

impl Default for BasicConfiguration {
    fn default() -> Self {
        Self {
            seed: Seed(random::get_seed()),
            default_difficulty: Difficulty::Normal,
            op_permission_level: PermissionLvl::Four,
            allow_nether: true,
            allow_end: true,
            hardcore: false,
            tps: 20.0,
            default_gamemode: GameMode::Survival,
            force_gamemode: false,
            scrub_ips: true,
            use_favicon: true,
            favicon_path: None,
            default_level_name: "world".to_string(),
            allow_chat_reports: false,
            white_list: false,
            enforce_whitelist: false,
            accepts_transfers: true,
            spawn_protection: 16,
        }
    }
}

impl BasicConfiguration {
    /// 返回服务器默认世界文件夹的路径。
    #[must_use]
    pub fn get_world_path(&self) -> PathBuf {
        PathBuf::from(&self.default_level_name)
    }

    /// 验证基础配置选项。
    pub const fn validate(&self) {}
}

impl AdvancedConfiguration {
    /// 验证高级配置选项。
    pub const fn validate(&self) {
        //self.resource_pack.validate();
    }
}

/// 用于从 TOML 文件加载并校验配置的 trait。
///
/// 提供加载、与默认值合并等的默认实现，
/// 并把缺失的值写回磁盘。还需要验证逻辑。
pub trait LoadConfiguration {
    /// 从给定目录加载配置。
    ///
    /// 如果目录不存在则创建目录，然后读取 TOML 文件，
    /// 与默认值合并，写入缺失字段，并校验结果。
    #[must_use]
    // NOTE: 日志记录器可能尚未就绪。
    #[expect(clippy::print_stdout)]
    fn load(config_dir: &Path) -> Self
    where
        Self: Sized + Default + Serialize + DeserializeOwned,
    {
        if !config_dir.exists() {
            debug!("正在创建新的配置根目录");
            let _ = fs::create_dir(config_dir);
        }

        let mut path = config_dir.join(Self::get_path());
        if !path.exists()
            && let Some(legacy) = Self::get_legacy_path()
        {
            let legacy_path = config_dir.join(legacy);
            if legacy_path.exists() {
                println!(
                    "正在加载旧版配置文件 {}；建议将其重命名为 {}。",
                    legacy_path.display(),
                    path.display()
                );
                path = legacy_path;
            }
        }

        let config = if path.exists() {
            let file_content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(err) => {
                    error!("无法读取配置文件 {}：{err}", path.display());
                    return Self::default();
                }
            };

            let parsed_toml_value: toml::Value = match toml::from_str(&file_content) {
                Ok(val) => val,
                Err(err) => {
                    error!(
                        "无法解析 TOML 文件 {}，原因：{err}。将使用默认配置。",
                        path.display()
                    );
                    return Self::default();
                }
            };

            let (merged_config, changed) = Self::merge_with_default_toml(parsed_toml_value);

            if changed {
                let file_name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |f| f.to_string_lossy().into_owned(),
                );
                println!("{file_name} 因缺少配置项已变更，缺失的值已用默认值补齐。");
                match toml::to_string(&merged_config) {
                    Ok(toml_str) => {
                        if let Err(err) = fs::write(&path, toml_str) {
                            warn!("无法将合并后的配置写入 {}，原因：{}", path.display(), err);
                        }
                    }
                    Err(err) => {
                        warn!("无法序列化 {} 的合并配置，原因：{}", path.display(), err);
                    }
                }
            }

            merged_config
        } else {
            let content = Self::default();
            match toml::to_string(&content) {
                Ok(toml_str) => {
                    if let Err(err) = fs::write(&path, toml_str) {
                        warn!("无法将默认配置写入 {:?}，原因：{}", path.display(), err);
                    }
                }
                Err(err) => {
                    warn!("无法序列化 {:?} 的默认配置，原因：{}", path.display(), err);
                }
            }

            content
        };

        config.validate();
        config
    }

    /// 将解析后的 TOML 值与默认配置合并。
    ///
    /// 返回合并后的配置，以及指示是否有值被填充的标志。
    #[must_use]
    fn merge_with_default_toml(parsed_toml: toml::Value) -> (Self, bool)
    where
        Self: Sized + Default + Serialize + DeserializeOwned,
    {
        let default_config = Self::default();

        let Ok(default_toml_value) = toml::Value::try_from(&default_config) else {
            return (default_config, false);
        };

        let (merged_value, changed) = Self::merge_toml_values(default_toml_value, parsed_toml);

        let config = merged_value.try_into().unwrap_or_else(|_| Self::default());

        (config, changed)
    }

    /// 递归合并两个 TOML 值。
    ///
    /// 基础层作为默认值；覆盖层会覆写其中的值。
    #[must_use]
    fn merge_toml_values(base: toml::Value, overlay: toml::Value) -> (toml::Value, bool) {
        match (base, overlay) {
            (toml::Value::Table(mut base_table), toml::Value::Table(overlay_table)) => {
                let mut changed = false;

                for key in base_table.keys() {
                    if !overlay_table.contains_key(key) {
                        changed = true;
                        break;
                    }
                }

                for (key, overlay_value) in overlay_table {
                    if let Some(base_value) = base_table.get(&key).cloned() {
                        let (merged_value, value_changed) =
                            Self::merge_toml_values(base_value, overlay_value);
                        base_table.insert(key, merged_value);
                        if value_changed {
                            changed = true;
                        }
                    } else {
                        base_table.insert(key, overlay_value);
                    }
                }
                (toml::Value::Table(base_table), changed)
            }
            (_, overlay) => (overlay, false),
        }
    }

    /// 返回配置文件相对于配置目录的路径。
    fn get_path() -> &'static Path;

    /// 主配置文件缺失时回退使用的旧版配置文件名
    /// (重命名迁移)。默认不设回退。
    #[must_use]
    fn get_legacy_path() -> Option<&'static Path> {
        None
    }

    /// 在加载或合并后验证配置。
    fn validate(&self);
}

#[cfg(test)]
mod tests {
    use toml::from_str;

    use super::BasicConfiguration;

    #[test]
    fn accepts_transfers_defaults_to_true() {
        let config: BasicConfiguration = BasicConfiguration::default();
        assert!(config.accepts_transfers);
    }

    #[test]
    fn accepts_transfers_reads_vanilla_aliases() {
        let config: BasicConfiguration = from_str("accepts-transfers = false").unwrap();
        assert!(!config.accepts_transfers);

        let config: BasicConfiguration = from_str("accepts_transfers = false").unwrap();
        assert!(!config.accepts_transfers);
    }
}
