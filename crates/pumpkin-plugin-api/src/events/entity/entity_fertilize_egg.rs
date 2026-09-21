use crate::wit::pumpkin::plugin::event::{EntityFertilizeEggEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an entity (turtle) fertilizes an egg.
pub struct EntityFertilizeEggEvent;
impl FromIntoEvent for EntityFertilizeEggEvent {
    const EVENT_TYPE: EventType = EventType::EntityFertilizeEggEvent;
    type Data = EntityFertilizeEggEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityFertilizeEggEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityFertilizeEggEvent(data)
    }
}
