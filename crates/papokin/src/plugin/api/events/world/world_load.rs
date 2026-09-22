use crate::world::World;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 世界被加载时发生的事件。
#[derive(Event, Clone)]
pub struct WorldLoadEvent {
    /// 已加载的世界。
    pub world: Arc<World>,
}

impl WorldLoadEvent {
    #[must_use]
    pub const fn new(world: Arc<World>) -> Self {
        Self { world }
    }
}

/// 世界被卸载时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldUnloadEvent {
    /// 正在卸载的世界。
    pub world: Arc<World>,
}

impl WorldUnloadEvent {
    #[must_use]
    pub const fn new(world: Arc<World>) -> Self {
        Self {
            world,
            cancelled: false,
        }
    }
}
