use crate::wit::papokin::plugin::event::{EntityTameEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被驯服时触发的事件。
pub struct EntityTameEvent;
impl FromIntoEvent for EntityTameEvent {
    const EVENT_TYPE: EventType = EventType::EntityTameEvent;
    type Data = EntityTameEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTameEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTameEvent(data)
    }
}
