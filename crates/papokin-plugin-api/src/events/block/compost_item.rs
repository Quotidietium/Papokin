use crate::wit::papokin::plugin::event::{CompostItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 物品被投入堆肥桶时触发的事件。
pub struct CompostItemEvent;
impl FromIntoEvent for CompostItemEvent {
    const EVENT_TYPE: EventType = EventType::CompostItemEvent;
    type Data = CompostItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CompostItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CompostItemEvent(data)
    }
}
