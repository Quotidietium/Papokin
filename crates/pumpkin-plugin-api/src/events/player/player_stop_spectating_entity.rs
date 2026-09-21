use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerStopSpectatingEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player stops spectating an entity. This event
/// is cancellable.
pub struct PlayerStopSpectatingEntityEvent;
impl FromIntoEvent for PlayerStopSpectatingEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStopSpectatingEntityEvent;
    type Data = PlayerStopSpectatingEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStopSpectatingEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStopSpectatingEntityEvent(data)
    }
}
