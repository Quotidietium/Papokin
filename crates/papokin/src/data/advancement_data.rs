use crate::entity::player::Player;
use crate::entity::player::advancement::{AdvancementDataError, PlayerAdvancement};
use papokin_data::Advancement;
use papokin_util::identifier::Identifier;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::slice;
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

/// 管理玩家进度，包括数据创建与保存。
pub struct AdvancementManager {
    pub advancement_path: PathBuf,
    pub save_enabled: bool,
}

impl AdvancementManager {
    /// 使用玩家数据路径创建新的 `AdvancementManager` 实例。
    pub fn new(player_data_path: impl Into<PathBuf>, save_enabled: bool) -> Self {
        let path = player_data_path.into().join("advancements");
        if !path.exists()
            && let Err(e) = create_dir_all(&path)
        {
            error!("创建玩家数据目录 {} 失败：{e}", path.display());
        }
        Self {
            advancement_path: path,
            save_enabled,
        }
    }

    /// 检索游戏中所有可用进度的列表。
    #[must_use]
    #[inline]
    pub fn get_advancements(&self) -> Vec<Identifier> {
        Advancement::get_identifier_list().to_vec()
    }

    /// 使用已配置的路径创建并返回一个新的 `PlayerAdvancement` 实例。
    #[inline]
    #[must_use]
    pub fn new_player_advancement(self: Arc<Self>, owner: Uuid) -> PlayerAdvancement {
        PlayerAdvancement::new(self, owner)
    }

    /// 保存所有给定玩家的进度。
    pub async fn save_all_players(
        &self,
        players: &[Arc<Player>],
    ) -> Result<(), AdvancementDataError> {
        if !self.save_enabled {
            return Ok(());
        }
        let mut to_write = Vec::with_capacity(players.len());
        for player in players {
            let guard = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let json = serde_json::to_string_pretty(&*guard).map_err(AdvancementDataError::Json)?;
            to_write.push((guard.path.clone(), json));
        }

        if let Err(e) = tokio::fs::create_dir_all(&self.advancement_path).await {
            error!("创建玩家进度目录失败：{e}");
            return Err(AdvancementDataError::Io(e));
        }
        for (path, json) in to_write {
            tokio::fs::write(&path, json)
                .await
                .map_err(AdvancementDataError::Io)?;
        }
        Ok(())
    }

    /// 保存指定玩家的进度。
    pub async fn save_player(&self, player: &Arc<Player>) -> Result<(), AdvancementDataError> {
        self.save_all_players(slice::from_ref(player)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn advancement_manager_new() {
        let path = PathBuf::from("test_data");
        let manager = AdvancementManager::new(path, true);
        assert_eq!(
            manager.advancement_path,
            PathBuf::from("test_data/advancements")
        );
    }

    #[test]
    fn get_advancement_path() {
        let path = PathBuf::from("world/playerdata");
        let manager = AdvancementManager::new(path, true);
        let advancement_path = manager.advancement_path;
        assert!(advancement_path.ends_with("advancements"));
    }
}
