use papokin_macros::Event;

/// 地图初始化时触发的事件。
#[derive(Event, Clone)]
pub struct MapInitializeEvent {
    /// 已初始化地图的 ID。
    pub map_id: i32,
}

impl MapInitializeEvent {
    #[must_use]
    pub const fn new(map_id: i32) -> Self {
        Self { map_id }
    }
}
