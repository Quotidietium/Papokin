use papokin_macros::Event;

/// 在按名称查找玩家档案之前发生的事件，
/// 允许插件提供缓存的资料。
///
/// 在当前宿主实现中，这是一个纯通知；该
/// 查找仍会继续进行（参见事件接线部分的集成说明
/// 报告）。此阶段尚不存在玩家对象，因此该事件不
/// 实现 `PlayerEvent`。
#[derive(Event, Clone)]
pub struct PreLookupProfileEvent {
    /// 正在查询的名称。
    pub name: String,
}

impl PreLookupProfileEvent {
    /// 创建 `PreLookupProfileEvent` 的新实例。
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
