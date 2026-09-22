use papokin_macros::{Event, cancellable};

/// 实体开始或停止使用鞘翅滑翔时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityToggleGlideEvent {
    /// 实体的 ID。
    pub entity_id: i32,

    /// 实体现在是否正在滑翔。
    pub is_gliding: bool,
}

impl EntityToggleGlideEvent {
    #[must_use]
    pub const fn new(entity_id: i32, is_gliding: bool) -> Self {
        Self {
            entity_id,
            is_gliding,
            cancelled: false,
        }
    }
}
