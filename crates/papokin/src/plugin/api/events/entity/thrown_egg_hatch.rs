use papokin_data::entity::EntityType;
use papokin_macros::Event;

/// 投掷出的鸡蛋决定是否孵化时发生的事件。
///
/// 不可取消；插件通过编辑 `will_hatch` 来改变结果，
/// `num_hatches` 与 `hatching_type`。
#[derive(Event, Clone)]
pub struct ThrownEggHatchEvent {
    /// 蛋实体的 ID。
    pub egg_id: i32,

    /// 蛋是否会孵化。
    pub will_hatch: bool,

    /// 从蛋中孵出的实体数量。
    pub num_hatches: u8,

    /// 正在孵化的实体类型。
    pub hatching_type: &'static EntityType,
}

impl ThrownEggHatchEvent {
    #[must_use]
    pub const fn new(
        egg_id: i32,
        will_hatch: bool,
        num_hatches: u8,
        hatching_type: &'static EntityType,
    ) -> Self {
        Self {
            egg_id,
            will_hatch,
            num_hatches,
            hatching_type,
        }
    }
}
