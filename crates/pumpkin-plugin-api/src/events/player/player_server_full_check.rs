use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerServerFullCheckEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player tries to join a full server. Setting
/// the result to `allowed` lets the player join anyway.
pub struct PlayerServerFullCheckEvent;
impl FromIntoEvent for PlayerServerFullCheckEvent {
    const EVENT_TYPE: EventType = EventType::PlayerServerFullCheckEvent;
    type Data = PlayerServerFullCheckEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerServerFullCheckEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerServerFullCheckEvent(data)
    }
}
