use crate::{
    entity::{NBTStorage, player::Player},
    server::Server,
};
use crossbeam::atomic::AtomicCell;
use papokin_inventory::screen_handler::ScreenHandler;
use papokin_nbt::compound::NbtCompound;
use papokin_world::data::player_data::{PlayerDataError, PlayerDataStorage};
use std::sync::Arc;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tracing::{debug, error};

/// 在服务器上下文中管理玩家数据的辅助工具。
///
/// 此结构体在整个服务器范围内提供对 `PlayerDataStorage` 和
/// 面向玩家处理的便捷方法。
pub struct ServerPlayerData {
    storage: Arc<PlayerDataStorage>,
    save_interval: Duration,
    last_save: AtomicCell<Instant>,
}

impl ServerPlayerData {
    /// 使用指定配置创建新的 `ServerPlayerData`。
    pub fn new(data_path: impl Into<PathBuf>, save_interval: Duration, enabled: bool) -> Self {
        Self {
            storage: Arc::new(PlayerDataStorage::new(data_path, enabled)),
            save_interval,
            last_save: AtomicCell::new(Instant::now()),
        }
    }

    /// 处理玩家离开服务器的情况。
    ///
    /// 此函数在玩家断开连接时保存其数据。
    ///
    /// # Arguments
    ///
    /// * `player` - 离开的玩家。
    ///
    /// # Returns
    ///
    /// 一个 Result，表示成功或发生的错误。
    pub fn handle_player_leave(&self, player: &Arc<Player>) -> Result<(), PlayerDataError> {
        player
            .player_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .on_closed(player.as_ref());
        player.on_handled_screen_closed();

        let mut nbt = NbtCompound::new();
        player.write_nbt(&mut nbt);

        self.storage.save_player_data(&player.gameprofile.id, nbt)?;
        Ok(())
    }

    /// 执行周期性维护任务。
    ///
    /// 此函数在服务器刻循环中被同步调用，用于检查
    /// 是否到了保存玩家数据的时间。
    pub fn tick(&self, server: &Server) {
        let now = Instant::now();

        // 仅根据 save_interval 定期保存玩家
        let last_save = self.last_save.load();
        let should_save = now.duration_since(last_save) >= self.save_interval;

        if should_save && self.storage.is_save_enabled() {
            self.last_save.store(now);
            // 定期对所有世界中的所有在线玩家做快照
            let mut snapshots = Vec::new();
            for world in server.worlds.load().iter() {
                for player in world.players.load().iter() {
                    let mut nbt = NbtCompound::new();
                    player.write_nbt(&mut nbt);
                    snapshots.push((player.gameprofile.id, nbt));
                }
            }

            if snapshots.is_empty() {
                return;
            }

            let storage = self.storage.clone();
            rayon::spawn(move || {
                for (uuid, nbt) in snapshots {
                    if let Err(e) = storage.save_player_data(&uuid, nbt) {
                        error!("保存玩家 {uuid} 的数据失败：{e}");
                    }
                }
                debug!("周期性玩家数据保存已完成");
            });
        }
    }

    /// 立即保存所有玩家的数据。
    ///
    /// 此函数会立即将所有在线玩家的数据保存到磁盘。
    /// 对服务器关停或备份操作很有用。
    pub fn save_all_players(&self, server: &Server) -> Result<(), PlayerDataError> {
        let mut total_players = 0;

        // 保存所有世界的玩家
        for world in server.worlds.load().iter() {
            for player in world.players.load().iter() {
                self.extract_data_and_save_player(player)?;
                total_players += 1;
            }
        }

        debug!("已保存 {total_players} 名在线玩家的数据");
        Ok(())
    }

    /// 加载玩家数据并将其应用到玩家身上。
    ///
    /// 此函数加载玩家的数据并将其应用到该玩家的 Player 实例。
    /// 对新玩家而言，它会无错误地创建默认数据。
    ///
    /// # Arguments
    ///
    /// * `player` - 要为其加载数据并应用到的玩家。
    ///
    /// # Returns
    ///
    /// 一个 Result，表示成功或发生的错误。
    pub fn load_data(&self, uuid: &uuid::Uuid) -> Result<Option<NbtCompound>, PlayerDataError> {
        let result = self.storage.load_player_data(uuid);

        match result {
            Ok((should_load, data)) => {
                if !should_load {
                    // 无数据可加载，继续使用默认数据
                    return Ok(None);
                }
                Ok(Some(data))
            }
            Err(e) => {
                if self.storage.is_save_enabled() {
                    // 仅在启用玩家数据保存时才记录为错误
                    error!("加载玩家 {uuid} 的数据时出错：{e}");
                } else {
                    // 否则只记录为 info，因为这是预期情况
                    debug!("不加载玩家 {uuid} 的数据（保存已禁用）");
                }
                // 即使出错也继续使用默认数据
                Ok(None)
            }
        }
    }

    /// 从玩家提取并保存数据。
    ///
    /// 此函数从玩家提取 NBT 数据并保存到磁盘。
    ///
    /// # Arguments
    ///
    /// * `player` - 要为其提取并保存数据的玩家。
    ///
    /// # Returns
    ///
    /// 一个 Result，表示成功或发生的错误。
    pub fn extract_data_and_save_player(&self, player: &Player) -> Result<(), PlayerDataError> {
        if !self.storage.is_save_enabled() {
            return Ok(());
        }

        let uuid = player.gameprofile.id;
        let mut nbt = NbtCompound::new();
        player.write_nbt(&mut nbt);

        self.storage.save_player_data(&uuid, nbt)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::data::player_server::ServerPlayerData;
    use papokin_nbt::compound::NbtCompound;
    use papokin_world::data::player_data::PlayerDataStorage;
    use std::time::Duration;
    use std::time::Instant;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn player_data_storage_new() {
        // 创建用于测试的临时目录
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let storage = PlayerDataStorage::new(path.clone(), true);

        assert_eq!(storage.get_data_path().as_path(), path.as_path());
        // Note: save_enabled 在你的实际代码中可能配置不同
    }

    #[tokio::test]
    async fn player_data_storage_get_player_data_path() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let storage = PlayerDataStorage::new(path.clone(), true);

        let uuid = Uuid::new_v4();
        let expected_path = path.join(format!("{uuid}.dat"));

        assert_eq!(storage.get_player_data_path(&uuid), expected_path);
    }

    #[tokio::test]
    async fn player_data_storage_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let storage = PlayerDataStorage::new(path, true); // 确保此测试已启用保存

        let uuid = Uuid::new_v4();

        // 创建测试数据
        let mut nbt = NbtCompound::new();
        nbt.put_string("TestKey", "TestValue".to_string());
        nbt.put_int("TestInt", 42);

        // 保存数据
        storage.save_player_data(&uuid, nbt).unwrap();

        // 加载数据
        let (load_success, loaded_nbt) = storage.load_player_data(&uuid).unwrap();

        assert!(load_success);
        assert_eq!(loaded_nbt.get_string("TestKey").unwrap(), "TestValue");
        assert_eq!(loaded_nbt.get_int("TestInt").unwrap(), 42);
    }

    #[tokio::test]
    async fn player_data_storage_load_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let storage = PlayerDataStorage::new(path, true); // 确保此测试已启用保存

        let uuid = Uuid::new_v4();

        // 尝试加载不存在的数据
        let (load_success, empty_nbt) = storage.load_player_data(&uuid).unwrap();

        assert!(!load_success);
        assert_eq!(empty_nbt.child_tags.len(), 0);
    }

    #[tokio::test]
    async fn player_data_storage_disabled() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let storage = PlayerDataStorage::new(path, false);

        let uuid = Uuid::new_v4();
        let mut nbt = NbtCompound::new();
        nbt.put_string("TestKey", "TestValue".to_string());

        // 保存应成功但不做任何事
        let save_result = storage.save_player_data(&uuid, nbt);
        assert!(save_result.is_ok());

        // 加载时应返回空数据
        let (load_success, empty_nbt) = storage.load_player_data(&uuid).unwrap();
        assert!(!load_success);
        assert_eq!(empty_nbt.child_tags.len(), 0);
    }

    #[tokio::test]
    async fn server_player_data_new() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        let save_interval = Duration::from_mins(5);

        let player_data = ServerPlayerData::new(path, save_interval, true);

        assert_eq!(player_data.save_interval, save_interval);
        assert!(
            Instant::now().duration_since(player_data.last_save.load()) < Duration::from_secs(1)
        );
    }

    #[tokio::test]
    async fn player_data_file_structure() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let uuid = Uuid::new_v4();
        let storage = PlayerDataStorage::new(path, true);

        // 创建并保存玩家数据
        let mut nbt = NbtCompound::new();
        nbt.put_string("name", "TestPlayer".to_string());
        nbt.put_int("level", 42);
        storage.save_player_data(&uuid, nbt).unwrap();

        // 验证文件存在
        let player_data_path = storage.get_player_data_path(&uuid);
        assert!(player_data_path.exists());

        // 再次加载并验证内容
        let (success, loaded_data) = storage.load_player_data(&uuid).unwrap();
        assert!(success);
        assert_eq!(loaded_data.get_string("name").unwrap(), "TestPlayer");
        assert_eq!(loaded_data.get_int("level").unwrap(), 42);
    }
}
