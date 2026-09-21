use crate::wit::pumpkin::plugin::event::{EntityJumpEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity jumps.
pub struct EntityJumpEvent;
impl FromIntoEvent for EntityJumpEvent {
    const EVENT_TYPE: EventType = EventType::EntityJumpEvent;
    type Data = EntityJumpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityJumpEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityJumpEvent(data)
    }
}
