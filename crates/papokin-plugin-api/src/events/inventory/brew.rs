use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BrewEventData, Event, EventType};

/// 药水在酿造台中酿造完成时触发的事件。
pub struct BrewEvent;
impl FromIntoEvent for BrewEvent {
    const EVENT_TYPE: EventType = EventType::BrewEvent;
    type Data = BrewEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BrewEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BrewEvent(data)
    }
}
