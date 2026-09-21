use crate::wit::pumpkin::plugin::event::{Event, EventType, LookupProfileEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player profile has been looked up by name.
pub struct LookupProfileEvent;
impl FromIntoEvent for LookupProfileEvent {
    const EVENT_TYPE: EventType = EventType::LookupProfileEvent;
    type Data = LookupProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LookupProfileEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LookupProfileEvent(data)
    }
}
