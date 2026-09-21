use crate::wit::pumpkin::plugin::event::{Event, EventType, Gs4QueryEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a GS4 query is received.
pub struct Gs4QueryEvent;
impl FromIntoEvent for Gs4QueryEvent {
    const EVENT_TYPE: EventType = EventType::Gs4QueryEvent;
    type Data = Gs4QueryEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::Gs4QueryEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::Gs4QueryEvent(data)
    }
}
