use crate::wit::papokin::plugin::event::{ClientTickEndEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家客户端完成一刻时触发的事件。这是一个
/// 高频通知。
pub struct ClientTickEndEvent;
impl FromIntoEvent for ClientTickEndEvent {
    const EVENT_TYPE: EventType = EventType::ClientTickEndEvent;
    type Data = ClientTickEndEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ClientTickEndEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ClientTickEndEvent(data)
    }
}
