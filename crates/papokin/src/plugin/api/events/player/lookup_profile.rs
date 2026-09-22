use papokin_macros::Event;
use uuid::Uuid;

/// 按名称查询到玩家档案后触发的事件。
///
/// `properties` 目前始终为空：名称查找只会解析
/// UUID，属性稍后由填充步骤获取。这是一个纯粹的
/// 通知，且不实现 `PlayerEvent`。
#[derive(Event, Clone)]
pub struct LookupProfileEvent {
    /// 被查询的名称。
    pub name: String,

    /// 解析出的档案的 UUID。
    pub player_uuid: Uuid,

    /// 已解析档案的名称，如果有的话。
    pub player_name: Option<String>,

    /// 以名称/值对形式表示的资料属性（目前始终为空）。
    pub properties: Vec<(String, String)>,
}

impl LookupProfileEvent {
    /// 创建 `LookupProfileEvent` 的新实例。
    pub fn new(
        name: impl Into<String>,
        player_uuid: Uuid,
        player_name: Option<String>,
        properties: Vec<(String, String)>,
    ) -> Self {
        Self {
            name: name.into(),
            player_uuid,
            player_name,
            properties,
        }
    }
}
