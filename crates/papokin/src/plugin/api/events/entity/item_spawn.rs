use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 物品实体在世界中生成时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ItemSpawnEvent {
    /// 物品实体的 ID。
    pub entity_id: i32,
    /// 物品实体的位置。
    pub position: Vector3<f64>,
    /// 物品注册表名称。
    pub item_name: String,
}

impl ItemSpawnEvent {
    #[must_use]
    pub const fn new(entity_id: i32, position: Vector3<f64>, item_name: String) -> Self {
        Self {
            entity_id,
            position,
            item_name,
            cancelled: false,
        }
    }
}
