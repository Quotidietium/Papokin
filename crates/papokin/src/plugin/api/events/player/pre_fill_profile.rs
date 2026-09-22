use papokin_macros::Event;
use uuid::Uuid;

/// 在玩家档案被填充其
/// 属性，从而允许插件提供缓存的 profile。
///
/// 处理器可用缓存数据填充 `properties`（以及 `player_name`）；
/// 当前宿主实现之后总是会执行获取，并且
/// 尚不会在预填充档案上短路（参见集成测试
/// 事件接线报告中的备注）。此时不存在玩家对象，
/// 阶段，因此该事件不实现 `PlayerEvent`。
#[derive(Event, Clone)]
pub struct PreFillProfileEvent {
    /// 档案的 UUID。
    pub player_uuid: Uuid,

    /// 档案的名称，如果已知的话。
    pub player_name: Option<String>,

    /// 以名称/值对形式表示的资料属性（可修改）。
    pub properties: Vec<(String, String)>,
}

impl PreFillProfileEvent {
    /// 创建 `PreFillProfileEvent` 的新实例。
    #[must_use]
    pub const fn new(
        player_uuid: Uuid,
        player_name: Option<String>,
        properties: Vec<(String, String)>,
    ) -> Self {
        Self {
            player_uuid,
            player_name,
            properties,
        }
    }
}
