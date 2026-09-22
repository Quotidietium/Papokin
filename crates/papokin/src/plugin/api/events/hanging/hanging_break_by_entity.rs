use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::EntityBase;

/// 悬挂实体被另一实体破坏时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct HangingBreakByEntityEvent {
    /// 被破坏的悬挂实体。
    pub entity: Arc<dyn EntityBase>,
    /// 破坏悬挂实体的实体。
    pub remover: Arc<dyn EntityBase>,
}

impl HangingBreakByEntityEvent {
    #[must_use]
    pub const fn new(entity: Arc<dyn EntityBase>, remover: Arc<dyn EntityBase>) -> Self {
        Self {
            entity,
            remover,
            cancelled: false,
        }
    }
}
