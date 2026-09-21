use crate::wit::pumpkin::plugin::event::{EndermanEscapeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an enderman escapes from combat.
pub struct EndermanEscapeEvent;
impl FromIntoEvent for EndermanEscapeEvent {
    const EVENT_TYPE: EventType = EventType::EndermanEscapeEvent;
    type Data = EndermanEscapeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EndermanEscapeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EndermanEscapeEvent(data)
    }
}
