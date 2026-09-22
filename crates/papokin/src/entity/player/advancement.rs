pub mod trigger;
mod visibility_evaluator;

use crate::data::advancement_data::AdvancementManager;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use indexmap::IndexMap;
use papokin_data::advancement_data::{
    AdvancementNode, AdvancementProgressData, AdvancementRequirement, AdvancementReward, Criteria,
};
use papokin_data::{ADVANCEMENT_TREE, Advancement};
use papokin_protocol::java::client::play::{
    CSelectAdvancementsTab, CSystemChatMessage, CUpdateAdvancements,
};
use papokin_util::identifier::Identifier;
use papokin_util::text::TextComponent;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::to_string_pretty;
use std::collections::{HashMap, HashSet};
use std::fs::read;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct CriterionProgress(pub Option<SystemTime>);

impl CriterionProgress {
    pub fn grant(&mut self) {
        self.0 = Some(SystemTime::now());
    }

    pub const fn revoke(&mut self) {
        self.0 = None;
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.0.is_some()
    }
}

/// 表示玩家在给定进度上的完成情况。
///
/// 跟踪该进度是否已全部完成。将来，
/// 这还会追踪特定条件的进度。
#[derive(Debug, Clone, Default)]
pub struct AdvancementProgress {
    /// 表示所有判据的不同进度，目前仅为布尔值
    pub criteria: HashMap<Arc<str>, CriterionProgress>,
    /// 进度被标记为完成所需的条件
    pub requirements: AdvancementRequirement,
}

impl AdvancementProgress {
    ///若该进度已完成，则返回 `true`。
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.requirements.test(|s| self.is_criterion_done(s))
    }

    /// 检查某个判据的标记是否已完成
    fn is_criterion_done(&self, criterion: &str) -> bool {
        self.criteria
            .get(criterion)
            .is_some_and(CriterionProgress::is_done)
    }

    ///若该进度有任何进展，则返回 `true`。目前仅在其完全完成时才返回。
    #[must_use]
    pub fn has_progress(&self) -> bool {
        for value in self.criteria.values() {
            if value.is_done() {
                return true;
            }
        }
        false
    }

    pub fn grant_progress(&mut self, name: &str) -> bool {
        if let Some(value) = self.criteria.get_mut(name)
            && !value.is_done()
        {
            value.grant();
            true
        } else {
            false
        }
    }

    pub fn revoke_progress(&mut self, name: &str) -> bool {
        if let Some(value) = self.criteria.get_mut(name)
            && value.is_done()
        {
            value.revoke();
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, requirements: AdvancementRequirement) {
        let names = requirements.names();
        self.criteria.retain(|key, _criterion| names.contains(key));
        for name in names {
            self.criteria.entry(name).or_default();
        }
        self.requirements = requirements;
    }

    #[inline]
    pub fn get_remaining_criteria(&self) -> impl Iterator<Item = Arc<str>> {
        self.criteria
            .iter()
            .filter(|&(_id, criterion)| !criterion.is_done())
            .map(|(id, _criterion)| id.clone())
    }

    #[inline]
    pub fn get_completed_criteria(&self) -> impl Iterator<Item = Arc<str>> {
        self.criteria
            .iter()
            .filter(|&(_id, criterion)| criterion.is_done())
            .map(|(id, _criterion)| id.clone())
    }
}

impl Serialize for AdvancementProgress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map: HashMap<&Arc<str>, &CriterionProgress> = self
            .criteria
            .iter()
            .filter(|(_key, criteria)| criteria.is_done())
            .collect();
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AdvancementProgress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let criteria = HashMap::<Arc<str>, CriterionProgress>::deserialize(deserializer)?;
        Ok(Self {
            criteria,
            requirements: AdvancementRequirement::default(),
        })
    }
}

#[derive(Clone, Default)]
pub struct AdvancementProgressMap {
    pub map: IndexMap<&'static Advancement, AdvancementProgress>,
}

impl AdvancementProgressMap {
    /// 获取给定进度当前进展的可变引用。若状态条目不存在则创建。
    pub fn get_mut_or_start_progress(
        &mut self,
        advancement: &'static Advancement,
    ) -> &mut AdvancementProgress {
        self.map.entry(advancement).or_insert_with(|| {
            let mut progress = AdvancementProgress::default();
            progress.update(AdvancementRequirement::from_const(advancement.requirements));
            progress
        })
    }

    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[inline]
    pub fn insert(&mut self, advancement: &'static Advancement, progress: AdvancementProgress) {
        self.map.insert(advancement, progress);
    }

    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// 管理玩家的进度集合。
///
/// 此方法处理已授予/已撤销进度的保存、加载和状态跟踪。
pub struct PlayerAdvancement {
    pub progress: AdvancementProgressMap,
    pub is_first_packet: bool,
    pub roots_to_update: HashSet<&'static AdvancementNode>,
    pub visible: HashSet<&'static Advancement>,
    pub progress_changed: HashSet<&'static Advancement>,
    pub manager: Arc<AdvancementManager>,
    pub path: PathBuf,
    pub last_selected_tab: Option<&'static Advancement>,
    /// 对拥有这些进度的玩家的弱引用。
    pub player: Weak<Player>,
}

/// 授予进度判据所产生的状态变更。
///
/// 完成时的副作用被有意推迟到调用者释放之后
/// 玩家的进度锁。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[must_use]
pub(crate) struct AdvancementAward {
    awarded: bool,
    completed: bool,
}

impl AdvancementAward {
    #[must_use]
    pub(crate) const fn awarded(self) -> bool {
        self.awarded
    }

    pub(crate) const fn combine(self, other: Self) -> Self {
        Self {
            awarded: self.awarded || other.awarded,
            completed: self.completed || other.completed,
        }
    }
}

/// 保存或加载进度数据时可能发生的错误。
#[derive(Debug, thiserror::Error)]
pub enum AdvancementDataError {
    #[error("IO error: {0}")]
    Io(std::io::Error),
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}

impl PlayerAdvancement {
    /// 创建 `PlayerAdvancement` 的新实例。
    #[must_use]
    pub fn new(manager: Arc<AdvancementManager>, uuid: Uuid) -> Self {
        Self {
            progress: AdvancementProgressMap::default(),
            path: manager.advancement_path.join(format!("{uuid}.json")),
            manager,
            player: Weak::new(),
            is_first_packet: true,
            roots_to_update: HashSet::default(),
            visible: HashSet::default(),
            progress_changed: HashSet::default(),
            last_selected_tab: None,
        }
    }

    /// 将 `PlayerAdvancement` 数据与给定玩家关联。
    pub fn set_player(&mut self, player: &Arc<Player>) {
        self.player = Arc::downgrade(player);
    }

    /// 返回此玩家是否启用了进度保存。
    #[must_use]
    #[inline]
    pub fn is_save_enabled(&self) -> bool {
        self.manager.save_enabled
    }

    ///从文件重新加载进度
    pub fn reload(&mut self) -> Result<(), AdvancementDataError> {
        //self.stopListening(); TODO
        self.progress.clear();
        self.visible.clear();
        self.roots_to_update.clear();
        self.progress_changed.clear();
        self.is_first_packet = true;
        self.last_selected_tab = None;
        self.load()
    }

    /// 将玩家的进度数据以 JSON 形式保存到磁盘。
    pub async fn save(&self) -> Result<(), AdvancementDataError> {
        if !self.is_save_enabled() {
            return Ok(());
        }
        let json = to_string_pretty(self).map_err(AdvancementDataError::Json)?;
        let Some(parent) = self.path.parent() else {
            return Ok(());
        };
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            error!("创建玩家进度目录失败：{e}");
            return Err(AdvancementDataError::Io(e));
        }
        tokio::fs::write(&self.path, json)
            .await
            .map_err(AdvancementDataError::Io)?;
        Ok(())
    }

    /// 从磁盘加载玩家的进度。
    pub fn load(&mut self) -> Result<(), AdvancementDataError> {
        if !self.path.exists() || !self.is_save_enabled() {
            return Ok(());
        }

        let json = read(&self.path).map_err(AdvancementDataError::Io)?;

        let loaded_data: HashMap<String, AdvancementProgress> =
            serde_json::from_slice(&json).map_err(AdvancementDataError::Json)?;

        self.progress.clear();
        for (advancement_id, mut progress) in loaded_data {
            if let Some(advancement_ref) = Advancement::from_minecraft_name(&advancement_id) {
                progress.update(AdvancementRequirement::from_const(
                    advancement_ref.requirements,
                ));
                self.progress.insert(advancement_ref, progress);
                self.progress_changed.insert(advancement_ref);
                self.mark_for_visibility_update(advancement_ref);
            } else {
                warn!("进度名称 {} 无效", advancement_id);
            }
        }
        Ok(())
    }

    fn mark_for_visibility_update(&mut self, advancement: &'static Advancement) {
        let node = ADVANCEMENT_TREE.get_node_from_id(&advancement.id);
        if let Some(node) = node {
            self.roots_to_update.insert(node.root());
        }
    }

    fn update_tree_visibility(
        &mut self,
        root: &AdvancementNode,
        added: &mut Vec<&'static Advancement>,
        removed: &mut Vec<Identifier>,
    ) {
        visibility_evaluator::evaluate_visibility(
            root,
            self,
            &mut |player_advancement, node| {
                player_advancement
                    .progress
                    .get_mut_or_start_progress(node.value)
                    .is_done()
            },
            &mut move |player_advancement, node, should_be_visible| {
                let advancement = node.value;
                if should_be_visible {
                    if player_advancement.visible.insert(advancement) {
                        added.push(advancement);
                        if player_advancement.progress.map.contains_key(advancement) {
                            player_advancement.progress_changed.insert(advancement);
                        }
                    }
                } else if player_advancement.visible.remove(advancement) {
                    removed.push(advancement.id.clone());
                }
            },
        );
    }

    /// 将所有待处理的进度状态下发到客户端。
    pub fn flush_dirty(&mut self, player: &Player, show_advancement: bool) {
        if self.is_first_packet || !self.roots_to_update.is_empty() {
            let mut progress: HashMap<Identifier, &AdvancementProgress> = HashMap::new();
            let mut added: Vec<&Advancement> = Vec::new();
            let mut removed: Vec<Identifier> = Vec::new();
            for root in self.roots_to_update.clone() {
                self.update_tree_visibility(root, &mut added, &mut removed);
            }
            self.roots_to_update.clear();
            for advancement in &self.progress_changed {
                if self.visible.contains(advancement) {
                    progress.insert(advancement.id.clone(), &self.progress.map[advancement]);
                }
            }
            self.progress_changed.clear();
            if !progress.is_empty() || !added.is_empty() || !removed.is_empty() {
                let parsed_progress: Vec<AdvancementProgressData> = progress
                    .into_iter()
                    .map(|(key, val)| AdvancementProgressData {
                        id: key,
                        progress: val
                            .criteria
                            .iter()
                            .map(|(key, val)| Criteria {
                                criterion_id: key.clone(),
                                achieve_date: val.0.map(|time| {
                                    time.duration_since(UNIX_EPOCH)
                                        .map_or(0, |d| d.as_millis() as i64)
                                }),
                            })
                            .collect(),
                    })
                    .collect();
                player.try_send_client_packet(&CUpdateAdvancements::new(
                    self.is_first_packet,
                    added,
                    parsed_progress,
                    removed,
                    show_advancement,
                ));
            }
        }
        self.is_first_packet = false;
    }

    /// 发放完成进度所关联的奖励（如经验）。
    pub fn grant_reward(player: &Arc<Player>, reward: &'static AdvancementReward) {
        player.add_experience_points(reward.experience);
    }

    /// 在玩家的进度状态被锁定期间记录一项判据的达成。
    ///
    /// 在释放锁之后调用 [`Self::finish_award`]，以便完成事件和奖励能够
    /// 安全地重入进度 API。
    pub(crate) fn award(
        &mut self,
        advancement: &'static Advancement,
        criterion: &str,
    ) -> AdvancementAward {
        let mut result = AdvancementAward::default();
        let progress = self.progress.get_mut_or_start_progress(advancement);
        let was_done = progress.is_done();
        if progress.grant_progress(criterion) {
            result.awarded = true;
            self.progress_changed.insert(advancement);
            if !was_done && progress.is_done() {
                result.completed = true;
            }
        }
        if !was_done && progress.is_done() {
            self.mark_for_visibility_update(advancement);
        }
        result
    }

    /// 执行已完成奖励中产生回调的副作用。
    ///
    /// 此方法只能在释放玩家的进度锁之后调用。
    pub(crate) fn finish_award(
        player: &Arc<Player>,
        advancement: &'static Advancement,
        result: AdvancementAward,
    ) {
        if !result.completed {
            return;
        }

        if let Some(server) = player.world().server.upgrade() {
            let mut event =
                crate::plugin::api::events::player::player_advancement_done::PlayerAdvancementDoneEvent::new(
                    player.clone(),
                    advancement.id.to_string(),
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        Self::grant_reward(player, advancement.reward);
        if let Some(display) = advancement.display
            && display.announce_to_chat
            && player
                .world()
                .level_info
                .load()
                .game_rules
                .show_advancement_messages
        {
            let player_name = player.get_display_name();
            let je_component = TextComponent::translate(
                display.frame_type.get_translation(),
                [player_name, advancement.name()],
            );
            let je_packet = CSystemChatMessage::new(&je_component, false);

            player.world().broadcast_packet_all(&je_packet);
        }
    }

    /// 撤销先前授予的进度，并清除其进度状态。
    pub fn revoke(&mut self, advancement: &'static Advancement, criterion: &str) -> bool {
        let mut result = false;
        let progress = self.progress.get_mut_or_start_progress(advancement);
        let was_done = progress.is_done();
        if progress.revoke_progress(criterion) {
            //TODO 监听器
            self.progress_changed.insert(advancement);
            result = true;
        }

        if was_done && !progress.is_done() {
            self.mark_for_visibility_update(advancement);
        }
        result
    }

    /// 设置玩家选中的进度标签页
    pub fn set_selected_tab(&mut self, advancement: Option<&'static Advancement>) {
        let old = self.last_selected_tab;
        if let Some(value) = advancement
            && value.is_root()
            && value.display.is_some()
        {
            self.last_selected_tab = advancement;
        } else {
            self.last_selected_tab = None;
        }
        if old != self.last_selected_tab
            && let Some(player) = self.player.upgrade()
        {
            let tab_id = self.last_selected_tab.map(|adv| adv.id.clone());
            player.try_send_client_packet(&CSelectAdvancementsTab::new(tab_id));
        }
    }
}

impl Serialize for PlayerAdvancement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let filtered_map: HashMap<&'static Advancement, &AdvancementProgress> = self
            .progress
            .map
            .iter()
            .filter(|(_key, value)| value.has_progress())
            .map(|(&key, val)| (key, val))
            .collect();
        let mut map = serializer.serialize_map(Some(filtered_map.len()))?;

        for (advancement, progress) in &filtered_map {
            map.serialize_entry(&advancement.id, &progress.criteria)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::advancement_data::AdvancementManager;
    use papokin_data::Advancement;
    use tempfile::tempdir;

    #[test]
    fn advancement_progress() {
        let mut criteria = HashMap::new();
        criteria.insert(Arc::from("testCriteria"), CriterionProgress::default());
        criteria.insert(Arc::from("testCriteria2"), CriterionProgress::default());
        let requirements = AdvancementRequirement {
            requirements: vec![
                vec![Arc::from("testCriteria")],
                vec![Arc::from("testCriteria2")],
            ],
        };
        let mut progress = AdvancementProgress {
            criteria,
            requirements,
        };
        assert!(!progress.is_done());
        assert!(!progress.has_progress());
        progress.grant_progress("testCriteria");
        assert!(!progress.is_done());
        assert!(progress.has_progress());
        progress.grant_progress("testCriteria2");
        assert!(progress.is_done());
        assert!(progress.has_progress());
    }

    #[test]
    fn new_player_advancement() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let pa = PlayerAdvancement::new(manager, id);
        assert!(pa.is_save_enabled());
        assert!(pa.is_first_packet);
        assert!(pa.roots_to_update.is_empty());
        assert!(pa.progress.is_empty());
    }

    #[test]
    fn get_or_start_progress() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);
        let adv = Advancement::STORY_ROOT;
        let progress = pa.progress.get_mut_or_start_progress(adv);
        assert!(
            !progress.is_done(),
            "New progress should not be marked done by default"
        );
    }

    #[test]
    fn revoke_advancement() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);
        let adv = Advancement::STORY_ROOT;
        {
            let progress_mut = pa.progress.get_mut_or_start_progress(adv);
            progress_mut.grant_progress("crafting_table");
        };
        assert!(pa.progress.get_mut_or_start_progress(adv).is_done());
        pa.revoke(adv, "crafting_table");
        assert!(!pa.progress.get_mut_or_start_progress(adv).is_done());
    }

    #[tokio::test]
    async fn save_advancement_progress() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);

        // 添加一些进度
        let adv = Advancement::STORY_ROOT;
        {
            let progress_mut = pa.progress.get_mut_or_start_progress(adv);
            progress_mut.grant_progress("crafting_table");
        };

        // 保存应成功
        assert!(pa.save().await.is_ok(), "Save should succeed");

        // 文件应存在
        assert!(pa.path.exists(), "Saved file should exist");

        // 内容应为合法 JSON
        let content = std::fs::read_to_string(&pa.path).unwrap();
        assert!(!content.is_empty(), "Saved file should not be empty");
        let _: HashMap<String, AdvancementProgress> =
            serde_json::from_str(&content).expect("保存的内容应为有效的 JSON");
    }

    #[tokio::test]
    async fn save_disabled() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), false));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);

        // 添加一些进度
        let adv = Advancement::STORY_ROOT;
        {
            let progress_mut = pa.progress.get_mut_or_start_progress(adv);
            progress_mut.grant_progress("crafting_table");
        };

        // 保存应返回 Ok，但不真正执行保存
        assert!(
            pa.save().await.is_ok(),
            "Save with disabled saving should return Ok"
        );
        assert!(
            !pa.path.exists(),
            "File should not be created when saving is disabled"
        );
    }

    #[tokio::test]
    async fn load_nonexistent_file() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);

        // 从不存在的文件加载应返回 Ok（而非错误）
        assert!(
            pa.load().is_ok(),
            "Loading from nonexistent file should return Ok"
        );
        assert!(pa.progress.is_empty(), "Advancements should remain empty");
    }

    #[tokio::test]
    async fn load_advancement_progress() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));

        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);
        // 创建包含进度数据的 JSON 文件
        let adv = Advancement::STORY_ROOT;
        let mut progress = AdvancementProgress::default();
        progress.update(AdvancementRequirement::from_const(adv.requirements));
        progress.grant_progress("crafting_table");
        let data = serde_json::json!({ adv.id.to_string():progress });
        std::fs::write(&pa.path, data.to_string()).unwrap();

        // 加载文件
        assert!(pa.load().is_ok(), "Load should succeed");

        // 验证该进度已被加载
        let loaded_progress = pa.progress.get_mut_or_start_progress(adv);
        assert!(
            loaded_progress.is_done(),
            "Loaded advancement should be marked complete"
        );
    }

    #[tokio::test]
    async fn save_load_roundtrip() {
        let temp_dir = tempdir().unwrap();

        // 创建并保存进度
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager.clone(), id);

        let adv = Advancement::STORY_ROOT;
        {
            let progress_mut = pa.progress.get_mut_or_start_progress(adv);
            progress_mut.grant_progress("crafting_table");
        };

        assert!(pa.save().await.is_ok(), "Save should succeed");

        // 将保存的进度加载到新实例中
        let mut pa_loaded = PlayerAdvancement::new(manager, id);
        assert!(pa_loaded.load().is_ok(), "Load should succeed");

        // 验证加载的数据与保存的数据一致
        let loaded_progress = pa_loaded.progress.get_mut_or_start_progress(adv);
        assert!(
            loaded_progress.is_done(),
            "Loaded progress should match saved progress"
        );
        assert_eq!(
            pa_loaded.progress.len(),
            pa.progress.len(),
            "Loaded advancements count should match"
        );
    }

    #[tokio::test]
    async fn load_invalid_advancement_id() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));

        // 创建包含非法进度 ID 的 JSON 文件
        let mut criteria = HashMap::new();
        criteria.insert(Arc::from("testCriteria"), CriterionProgress::default());
        let requirements = AdvancementRequirement {
            requirements: vec![vec![Arc::from("testCriteria")]],
        };
        let progress = AdvancementProgress {
            criteria,
            requirements,
        };
        let data = serde_json::json!({
            "invalid_advancement_id_12345": progress
        });
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);
        std::fs::write(&pa.path, data.to_string()).unwrap();

        // 加载仍应成功，但跳过无效条目

        assert!(
            pa.load().is_ok(),
            "Load should succeed even with invalid IDs"
        );
        assert!(
            pa.progress.is_empty(),
            "Invalid advancements should be skipped"
        );
    }

    #[tokio::test]
    async fn save_multiple_advancements() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), true));
        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);

        // 添加多个进度
        let adv1 = Advancement::STORY_ROOT;
        let adv2 = Advancement::NETHER_ROOT;

        {
            let progress_mut1 = pa.progress.get_mut_or_start_progress(adv1);
            progress_mut1.grant_progress("crafting_table");
        };
        {
            let progress_mut2 = pa.progress.get_mut_or_start_progress(adv2);
            progress_mut2.grant_progress("entered_nether");
        };

        assert!(pa.save().await.is_ok(), "Save should succeed");

        // 验证两者均已保存
        let content = std::fs::read_to_string(&pa.path).unwrap();
        let saved_data: HashMap<String, AdvancementProgress> =
            serde_json::from_str(&content).unwrap();
        assert_eq!(saved_data.len(), 2, "Should have saved both advancements");
    }

    #[tokio::test]
    async fn ignore_loading() {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(AdvancementManager::new(temp_dir.path(), false));

        let id = Uuid::new_v4();
        let mut pa = PlayerAdvancement::new(manager, id);
        // 创建包含进度数据的 JSON 文件
        let adv = Advancement::STORY_ROOT;
        let data = serde_json::json!({ adv.id.to_string(): { "complete": true } });
        std::fs::write(&pa.path, data.to_string()).unwrap();

        //尝试加载文件
        assert!(pa.load().is_ok(), "Load should succeed");

        // 验证该进度未被加载
        assert!(
            pa.progress.is_empty(),
            "The advancement shouldn't have been loaded"
        );
    }
}
