use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerUntrackEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs when an entity stops being tracked by a player.
pub struct PlayerUntrackEntityEvent;
impl FromIntoEvent for PlayerUntrackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerUntrackEntityEvent;
    type Data = PlayerUntrackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerUntrackEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerUntrackEntityEvent(data)
    }
}
