use crate::wit::pumpkin::plugin::event::{EntityMoveEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity moves.
pub struct EntityMoveEvent;
impl FromIntoEvent for EntityMoveEvent {
    const EVENT_TYPE: EventType = EventType::EntityMoveEvent;
    type Data = EntityMoveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityMoveEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityMoveEvent(data)
    }
}
