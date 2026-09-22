use papokin_macros::{Event, cancellable};

/// 药水状态效果被施加到实体上时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPotionEffectEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 效果名称。
    pub effect_name: String,
    /// 持续时间（刻）。
    pub duration: i32,
    /// 放大等级。
    pub amplifier: u8,
}

impl EntityPotionEffectEvent {
    #[must_use]
    pub const fn new(entity_id: i32, effect_name: String, duration: i32, amplifier: u8) -> Self {
        Self {
            entity_id,
            effect_name,
            duration,
            amplifier,
            cancelled: false,
        }
    }
}
