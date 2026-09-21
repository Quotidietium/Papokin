use crate::wit::pumpkin::plugin::event::{CompostItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an item is inserted into a composter.
pub struct CompostItemEvent;
impl FromIntoEvent for CompostItemEvent {
    const EVENT_TYPE: EventType = EventType::CompostItemEvent;
    type Data = CompostItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CompostItemEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CompostItemEvent(data)
    }
}
