use std::{env, fs, path::Path, sync::RwLock};

use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

const DATA_FOLDER: &str = "data/";

pub mod op;

pub mod advancement_data;
pub mod banlist_serializer;
pub mod banned_ip;
pub mod banned_player;
pub mod datapack;
pub mod player_server;
pub mod usercache;
pub mod whitelist;

pub struct VanillaData {
    pub banned_ip_list: RwLock<banned_ip::BannedIpList>,
    pub banned_player_list: RwLock<banned_player::BannedPlayerList>,
    pub operator_config: RwLock<op::OperatorConfig>,
    pub user_cache: RwLock<usercache::UserCache>,
    pub whitelist_config: RwLock<whitelist::WhitelistConfig>,
}

impl VanillaData {
    #[must_use]
    pub fn load() -> Self {
        Self {
            banned_ip_list: RwLock::new(banned_ip::BannedIpList::load()),
            banned_player_list: RwLock::new(banned_player::BannedPlayerList::load()),
            operator_config: RwLock::new(op::OperatorConfig::load()),
            user_cache: RwLock::new(usercache::UserCache::load()),
            whitelist_config: RwLock::new(whitelist::WhitelistConfig::load()),
        }
    }
}

pub trait LoadJSONConfiguration {
    #[must_use]
    fn load() -> Self
    where
        Self: Sized + Default + Serialize + for<'de> Deserialize<'de>,
    {
        let exe_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let data_dir = exe_dir.join(DATA_FOLDER);
        if !data_dir.exists() {
            debug!("正在创建新的数据根目录");
            let _ = fs::create_dir(&data_dir);
        }
        let path = data_dir.join(Self::get_path());

        let config = if path.exists() {
            let file_content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(err) => {
                    error!("无法读取配置文件 {}：{err}", path.display());
                    return Self::default();
                }
            };

            match serde_json::from_str(&file_content) {
                Ok(c) => c,
                Err(err) => {
                    error!(
                        "无法解析数据配置 {}。原因：{err}。回退到默认值。",
                        path.display()
                    );
                    Self::default()
                }
            }
        } else {
            let content = Self::default();

            if let Ok(json_str) = serde_json::to_string_pretty(&content) {
                let _ = fs::write(&path, json_str);
            }

            content
        };

        config.validate();
        config
    }

    fn get_path() -> &'static Path;

    fn validate(&self);
}

pub trait SaveJSONConfiguration: LoadJSONConfiguration {
    fn save(&self)
    where
        Self: Sized + Default + Serialize + for<'de> Deserialize<'de>,
    {
        let exe_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let data_dir = exe_dir.join(DATA_FOLDER);
        if !data_dir.exists() {
            debug!("正在创建新的数据根目录");
            let _ = fs::create_dir(&data_dir);
        }
        let path = data_dir.join(Self::get_path());

        let content = match serde_json::to_string_pretty(self) {
            Ok(content) => content,
            Err(err) => {
                warn!(
                    "无法将管理员数据配置序列化到 {}。原因：{err}",
                    path.display()
                );
                return;
            }
        };

        if let Err(err) = std::fs::write(&path, content) {
            warn!("无法写入管理员配置 {}。原因：{err}", path.display());
        }
    }
}
