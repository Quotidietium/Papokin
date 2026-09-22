use papokin_macros::Event;

/// 收到 `GS4`（`GameSpy` 4）查询时发生的事件。
///
/// 这是一个纯通知；`data` 携带的键值对已经
/// 为响应收集（当前实现中为只读）。
/// 此事件不涉及玩家对象，因此未实现
/// `PlayerEvent`。
#[derive(Event, Clone)]
pub struct Gs4QueryEvent {
    /// 查询类型（`basic` 或 `full`）。
    pub query_type: String,

    /// 查询者的地址。
    pub querier_address: String,

    /// 以键/值对形式表示的响应数据。
    pub data: Vec<(String, String)>,
}

impl Gs4QueryEvent {
    /// 创建新的 `Gs4QueryEvent` 实例。
    pub fn new(
        query_type: impl Into<String>,
        querier_address: impl Into<String>,
        data: Vec<(String, String)>,
    ) -> Self {
        Self {
            query_type: query_type.into(),
            querier_address: querier_address.into(),
            data,
        }
    }
}
