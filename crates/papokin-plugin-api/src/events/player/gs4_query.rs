use crate::wit::papokin::plugin::event::{Event, EventType, Gs4QueryEventData};

use super::super::FromIntoEvent;

/// 收到 GS4 query 时触发的事件。
pub struct Gs4QueryEvent;
impl FromIntoEvent for Gs4QueryEvent {
    const EVENT_TYPE: EventType = EventType::Gs4QueryEvent;
    type Data = Gs4QueryEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::Gs4QueryEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::Gs4QueryEvent(data)
    }
}
