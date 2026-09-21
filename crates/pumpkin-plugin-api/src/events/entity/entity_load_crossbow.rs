use crate::wit::pumpkin::plugin::event::{EntityLoadCrossbowEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity loads a crossbow with projectiles.
pub struct EntityLoadCrossbowEvent;
impl FromIntoEvent for EntityLoadCrossbowEvent {
    const EVENT_TYPE: EventType = EventType::EntityLoadCrossbowEvent;
    type Data = EntityLoadCrossbowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityLoadCrossbowEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityLoadCrossbowEvent(data)
    }
}
