use crate::wit::papokin::plugin::event::{BrewingStartEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 酿造台开始酿造时触发的事件。
pub struct BrewingStartEvent;
impl FromIntoEvent for BrewingStartEvent {
    const EVENT_TYPE: EventType = EventType::BrewingStartEvent;
    type Data = BrewingStartEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BrewingStartEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BrewingStartEvent(data)
    }
}
