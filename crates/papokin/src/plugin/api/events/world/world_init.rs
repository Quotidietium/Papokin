use crate::world::World;
use papokin_macros::Event;
use std::sync::Arc;

/// 世界初始化时发生的事件。
#[derive(Event, Clone)]
pub struct WorldInitEvent {
    /// 正在初始化的世界。
    pub world: Arc<World>,
}

impl WorldInitEvent {
    #[must_use]
    pub const fn new(world: Arc<World>) -> Self {
        Self { world }
    }
}
