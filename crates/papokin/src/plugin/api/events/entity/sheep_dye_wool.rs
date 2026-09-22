use papokin_macros::{Event, cancellable};

/// 绵羊的羊毛被染色时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SheepDyeWoolEvent {
    /// 绵羊的 ID。
    pub entity_id: i32,

    /// 新的染料颜色索引。
    pub dye_color: u8,

    /// 给羊染色的玩家 ID（如果有的话）。
    pub player_id: Option<i32>,
}
