use crate::wit::pumpkin::plugin::event::{Event, EventType, PreFillProfileEventData};

use super::super::FromIntoEvent;

/// An event that occurs before a player profile is filled, allowing a cached
/// profile to be supplied via the properties.
pub struct PreFillProfileEvent;
impl FromIntoEvent for PreFillProfileEvent {
    const EVENT_TYPE: EventType = EventType::PreFillProfileEvent;
    type Data = PreFillProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreFillProfileEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreFillProfileEvent(data)
    }
}
