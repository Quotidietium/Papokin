use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体受到来自方块的伤害时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageByBlockEvent {
    /// 受到伤害的实体 ID。
    pub entity_id: i32,

    /// 造成伤害的方块的位置（如果已知）。
    pub damager_pos: Option<BlockPos>,

    /// 造成的伤害量。
    pub damage: f32,

    /// 伤害原因。
    pub cause: String,
}
