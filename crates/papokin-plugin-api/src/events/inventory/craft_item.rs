use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{CraftItemEventData, Event, EventType};

/// 物品被合成时触发的事件。
pub struct CraftItemEvent;
impl FromIntoEvent for CraftItemEvent {
    const EVENT_TYPE: EventType = EventType::CraftItemEvent;
    type Data = CraftItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CraftItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CraftItemEvent(data)
    }
}
