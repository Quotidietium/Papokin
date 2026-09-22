use papokin_macros::{Event, cancellable};

/// 世界被保存时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WorldSaveEvent {
    /// 世界名称。
    pub world_name: String,
}

impl WorldSaveEvent {
    #[must_use]
    pub const fn new(world_name: String) -> Self {
        Self {
            world_name,
            cancelled: false,
        }
    }
}
