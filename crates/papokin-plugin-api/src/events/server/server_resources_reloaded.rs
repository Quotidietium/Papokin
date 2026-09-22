use crate::wit::papokin::plugin::event::{Event, EventType, ServerResourcesReloadedEventData};

use super::super::FromIntoEvent;

/// 服务器资源（数据包）重载时触发的事件。
pub struct ServerResourcesReloadedEvent;
impl FromIntoEvent for ServerResourcesReloadedEvent {
    const EVENT_TYPE: EventType = EventType::ServerResourcesReloadedEvent;
    type Data = ServerResourcesReloadedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerResourcesReloadedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerResourcesReloadedEvent(data)
    }
}
