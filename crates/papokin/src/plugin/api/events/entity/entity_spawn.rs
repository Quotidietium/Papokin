use crate::world::World;
use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;
use std::sync::Arc;

/// 实体生成到世界中时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntitySpawnEvent {
    /// 被生成实体的 ID。
    pub entity_id: i32,

    /// 实体类型的注册表名称。
    pub entity_type: String,

    /// 实体生成的位置。
    pub position: Vector3<f64>,

    /// 实体生成的世界。
    pub world: Arc<World>,
}

impl EntitySpawnEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        entity_type: String,
        position: Vector3<f64>,
        world: Arc<World>,
    ) -> Self {
        Self {
            entity_id,
            entity_type,
            position,
            world,
            cancelled: false,
        }
    }
}
