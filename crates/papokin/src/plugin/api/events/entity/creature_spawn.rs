use crate::world::World;
use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

/// 生物生成时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CreatureSpawnEvent {
    /// 被生成生物的 ID。
    pub entity_id: i32,

    /// 实体类型的注册表名称。
    pub entity_type: String,

    /// 生物生成的位置。
    pub position: Vector3<f64>,

    /// 生物生成的世界。
    pub world: Arc<World>,

    /// 生物生成的原因。
    pub spawn_reason: String,
}
