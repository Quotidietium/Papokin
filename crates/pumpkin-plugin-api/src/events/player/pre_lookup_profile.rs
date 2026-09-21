use crate::wit::pumpkin::plugin::event::{Event, EventType, PreLookupProfileEventData};

use super::super::FromIntoEvent;

/// An event that occurs before a player profile is looked up by name.
pub struct PreLookupProfileEvent;
impl FromIntoEvent for PreLookupProfileEvent {
    const EVENT_TYPE: EventType = EventType::PreLookupProfileEvent;
    type Data = PreLookupProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreLookupProfileEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreLookupProfileEvent(data)
    }
}
