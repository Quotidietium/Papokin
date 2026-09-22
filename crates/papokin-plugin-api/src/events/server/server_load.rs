use crate::wit::papokin::plugin::event::{Event, EventType, ServerLoadEventData};

use super::super::FromIntoEvent;

/// 服务器完成加载后触发的事件。
///
/// 关联的 [`ServerLoadEventData`] 包含加载原因：
/// 启动或服务器整体重载时。
pub struct ServerLoadEvent;
impl FromIntoEvent for ServerLoadEvent {
    const EVENT_TYPE: EventType = EventType::ServerLoadEvent;
    type Data = ServerLoadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerLoadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerLoadEvent(data)
    }
}
