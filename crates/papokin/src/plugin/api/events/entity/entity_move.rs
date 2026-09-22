use papokin_macros::Event;
use papokin_util::math::vector3::Vector3;

/// 实体移动时发生的事件。
///
/// 此事件频率极高；服务器仅在必要时才分发它，
/// 某个插件已为它注册了处理器。
#[derive(Event, Clone)]
pub struct EntityMoveEvent {
    /// 移动的实体 ID。
    pub entity_id: i32,

    /// 实体移动前的位置。
    pub from_position: Vector3<f64>,

    /// 实体移动后的位置。
    pub to_position: Vector3<f64>,
}

impl EntityMoveEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            entity_id,
            from_position,
            to_position,
        }
    }
}
