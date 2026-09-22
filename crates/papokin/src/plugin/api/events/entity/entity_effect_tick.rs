use papokin_macros::{Event, cancellable};

/// 实体身上每个活跃药水效果每刻触发一次的事件。
///
/// 此事件是高频事件；服务器仅在插件需要时才分发它，
/// 已为其注册了处理程序。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityEffectTickEvent {
    /// 效果正在生效的实体 ID。
    pub entity_id: i32,

    /// 效果类型的标识符（例如 `minecraft:speed`）。
    pub effect_type: String,

    /// 效果的倍率。
    pub amplifier: i32,

    /// 效果的剩余时长（以刻为单位）。
    pub duration: i32,
}

impl EntityEffectTickEvent {
    #[must_use]
    pub const fn new(entity_id: i32, effect_type: String, amplifier: i32, duration: i32) -> Self {
        Self {
            entity_id,
            effect_type,
            amplifier,
            duration,
            cancelled: false,
        }
    }
}
