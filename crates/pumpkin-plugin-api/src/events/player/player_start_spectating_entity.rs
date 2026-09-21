use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerStartSpectatingEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player starts spectating an entity. This event
/// is cancellable.
pub struct PlayerStartSpectatingEntityEvent;
impl FromIntoEvent for PlayerStartSpectatingEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStartSpectatingEntityEvent;
    type Data = PlayerStartSpectatingEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStartSpectatingEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStartSpectatingEntityEvent(data)
    }
}
