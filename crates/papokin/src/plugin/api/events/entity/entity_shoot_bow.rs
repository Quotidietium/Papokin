use papokin_macros::{Event, cancellable};

/// 实体射出弓或弩时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityShootBowEvent {
    /// 发射实体的 ID。
    pub entity_id: i32,

    /// 武器物品的注册表名称。
    pub weapon_name: String,

    /// 射击力度/速度系数。
    pub force: f32,
}

impl EntityShootBowEvent {
    #[must_use]
    pub const fn new(entity_id: i32, weapon_name: String, force: f32) -> Self {
        Self {
            entity_id,
            weapon_name,
            force,
            cancelled: false,
        }
    }
}
