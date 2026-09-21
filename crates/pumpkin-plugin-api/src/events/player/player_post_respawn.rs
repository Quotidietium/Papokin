use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerPostRespawnEventData};

use super::super::FromIntoEvent;

/// An event that occurs after a player respawns.
pub struct PlayerPostRespawnEvent;
impl FromIntoEvent for PlayerPostRespawnEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPostRespawnEvent;
    type Data = PlayerPostRespawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPostRespawnEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPostRespawnEvent(data)
    }
}
