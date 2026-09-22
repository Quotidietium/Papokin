use crate::wit::papokin::plugin::event::{CrafterCraftEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 合成器合成物品时触发的事件。
pub struct CrafterCraftEvent;
impl FromIntoEvent for CrafterCraftEvent {
    const EVENT_TYPE: EventType = EventType::CrafterCraftEvent;
    type Data = CrafterCraftEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CrafterCraftEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CrafterCraftEvent(data)
    }
}
