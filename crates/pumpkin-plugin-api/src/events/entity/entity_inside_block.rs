use crate::wit::pumpkin::plugin::event::{EntityInsideBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered every tick while an entity is inside a block.
pub struct EntityInsideBlockEvent;
impl FromIntoEvent for EntityInsideBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityInsideBlockEvent;
    type Data = EntityInsideBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityInsideBlockEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityInsideBlockEvent(data)
    }
}
