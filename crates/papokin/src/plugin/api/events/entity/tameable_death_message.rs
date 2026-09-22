use papokin_macros::Event;
use papokin_util::text::TextComponent;

/// 可驯服实体死亡并产生死亡消息时发生的事件
/// 创建。插件可以替换该消息。
#[derive(Event, Clone)]
pub struct TameableDeathMessageEvent {
    /// 死亡的可驯服实体的 ID。
    pub entity_id: i32,

    /// 要广播的死亡消息。
    pub death_message: TextComponent,
}

impl TameableDeathMessageEvent {
    #[must_use]
    pub const fn new(entity_id: i32, death_message: TextComponent) -> Self {
        Self {
            entity_id,
            death_message,
        }
    }
}
