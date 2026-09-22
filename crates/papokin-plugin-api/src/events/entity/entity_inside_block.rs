use crate::wit::papokin::plugin::event::{EntityInsideBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体处于方块内时每刻触发的事件。
pub struct EntityInsideBlockEvent;
impl FromIntoEvent for EntityInsideBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityInsideBlockEvent;
    type Data = EntityInsideBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityInsideBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityInsideBlockEvent(data)
    }
}
