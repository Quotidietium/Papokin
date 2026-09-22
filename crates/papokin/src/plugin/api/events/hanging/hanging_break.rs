use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::EntityBase;

/// 悬挂式实体（例如画、物品展示框）被破坏时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct HangingBreakEvent {
    /// 被破坏的悬挂实体。
    pub entity: Arc<dyn EntityBase>,
    /// 导致破坏的实体（如有）。
    pub remover: Option<Arc<dyn EntityBase>>,
}

impl HangingBreakEvent {
    #[must_use]
    pub const fn new(entity: Arc<dyn EntityBase>, remover: Option<Arc<dyn EntityBase>>) -> Self {
        Self {
            entity,
            remover,
            cancelled: false,
        }
    }
}
