use crate::wit::papokin::plugin::event::{EntityBreedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 两个实体繁殖时触发的事件。
pub struct EntityBreedEvent;
impl FromIntoEvent for EntityBreedEvent {
    const EVENT_TYPE: EventType = EventType::EntityBreedEvent;
    type Data = EntityBreedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityBreedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityBreedEvent(data)
    }
}
