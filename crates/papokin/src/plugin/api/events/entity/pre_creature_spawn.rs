use std::sync::Arc;

use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

use crate::world::World;

/// 生物生成前发生的事件，允许提前过滤。
#[cancellable]
#[derive(Event, Clone)]
pub struct PreCreatureSpawnEvent {
    /// 生成位置。
    pub position: Vector3<f64>,

    /// 生物生成所在的世界。
    pub world: Arc<World>,

    /// 正在生成的实体类型的标识符（例如 `minecraft:zombie`）。
    pub entity_type: String,

    /// 生成原因（例如 `NATURAL`、`CHUNK_GENERATION`）。
    pub reason: String,
}

impl PreCreatureSpawnEvent {
    #[must_use]
    pub const fn new(
        position: Vector3<f64>,
        world: Arc<World>,
        entity_type: String,
        reason: String,
    ) -> Self {
        Self {
            position,
            world,
            entity_type,
            reason,
            cancelled: false,
        }
    }
}
