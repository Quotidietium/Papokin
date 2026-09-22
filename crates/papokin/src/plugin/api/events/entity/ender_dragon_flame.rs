use papokin_macros::{Event, cancellable};

/// 末影龙喷吐龙息并产生范围效果区域时发生的事件
/// 效果云。
#[cancellable]
#[derive(Event, Clone)]
pub struct EnderDragonFlameEvent {
    /// 末影龙实体的 ID。
    pub entity_id: i32,

    /// 龙息产生的区域效果云 ID。
    pub area_effect_cloud_id: i32,
}

impl EnderDragonFlameEvent {
    #[must_use]
    pub const fn new(entity_id: i32, area_effect_cloud_id: i32) -> Self {
        Self {
            entity_id,
            area_effect_cloud_id,
            cancelled: false,
        }
    }
}
