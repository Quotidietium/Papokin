use papokin_macros::{Event, cancellable};
use papokin_util::math::vector3::Vector3;

/// 实体开始寻路前往目标时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPathfindEvent {
    /// 寻路实体的 ID。
    pub entity_id: i32,

    /// 路径目标实体的 ID（如有）。
    pub target_id: Option<i32>,

    /// 以位置列表形式给出的计算路径。
    pub path: Vec<Vector3<f64>>,
}

impl EntityPathfindEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: Option<i32>, path: Vec<Vector3<f64>>) -> Self {
        Self {
            entity_id,
            target_id,
            path,
            cancelled: false,
        }
    }
}
