use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerClientOptionsChangeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player's client options change. This is a
/// Java-protocol event.
pub struct PlayerClientOptionsChangeEvent;
impl FromIntoEvent for PlayerClientOptionsChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerClientOptionsChangeEvent;
    type Data = PlayerClientOptionsChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerClientOptionsChangeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerClientOptionsChangeEvent(data)
    }
}
