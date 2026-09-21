use crate::wit::pumpkin::plugin::event::{Event, EventType, TameableDeathMessageEventData};

use super::super::FromIntoEvent;

/// Event triggered when a tameable entity dies and its death message is created.
pub struct TameableDeathMessageEvent;
impl FromIntoEvent for TameableDeathMessageEvent {
    const EVENT_TYPE: EventType = EventType::TameableDeathMessageEvent;
    type Data = TameableDeathMessageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TameableDeathMessageEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TameableDeathMessageEvent(data)
    }
}
