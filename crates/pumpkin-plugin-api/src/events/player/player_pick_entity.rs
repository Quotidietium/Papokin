use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerPickEntityEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player picks an entity (middle click) and
/// receives an item. This event is cancellable.
pub struct PlayerPickEntityEvent;
impl FromIntoEvent for PlayerPickEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPickEntityEvent;
    type Data = PlayerPickEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPickEntityEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPickEntityEvent(data)
    }
}
