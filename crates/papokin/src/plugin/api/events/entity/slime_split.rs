use papokin_macros::{Event, cancellable};

/// 史莱姆分裂成更小的史莱姆时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SlimeSplitEvent {
    /// 父史莱姆实体的 ID。
    pub entity_id: i32,

    /// 生成的较小史莱姆的数量。
    pub count: i32,
}
