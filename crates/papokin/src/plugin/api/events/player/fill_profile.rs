use papokin_macros::Event;
use uuid::Uuid;

/// 玩家档案已填充好其
/// 属性（从认证服务器获取）。
///
/// 这是一个纯通知；此时还没有玩家对象，因此该事件
/// 未实现 `PlayerEvent`。
#[derive(Event, Clone)]
pub struct FillProfileEvent {
    /// 档案的 UUID。
    pub player_uuid: Uuid,

    /// 档案的名称，如果已解析的话。
    pub player_name: Option<String>,

    /// 以名称/值对形式表示的资料属性（例如 `textures`）。
    pub properties: Vec<(String, String)>,
}

impl FillProfileEvent {
    /// 创建新的 `FillProfileEvent` 实例。
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
