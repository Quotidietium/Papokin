use crate::wit::papokin::plugin::event::{EntityResurrectEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体复活时触发的事件。
pub struct EntityResurrectEvent;
impl FromIntoEvent for EntityResurrectEvent {
    const EVENT_TYPE: EventType = EventType::EntityResurrectEvent;
    type Data = EntityResurrectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityResurrectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityResurrectEvent(data)
    }
}
