use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerStopUsingItemEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player stops using an item (e.g. releases a
/// bow).
pub struct PlayerStopUsingItemEvent;
impl FromIntoEvent for PlayerStopUsingItemEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStopUsingItemEvent;
    type Data = PlayerStopUsingItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStopUsingItemEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStopUsingItemEvent(data)
    }
}
