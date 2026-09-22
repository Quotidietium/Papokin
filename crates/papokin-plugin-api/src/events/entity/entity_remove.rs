use crate::wit::papokin::plugin::event::{EntityRemoveEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体从世界移除时触发的事件。
pub struct EntityRemoveEvent;
impl FromIntoEvent for EntityRemoveEvent {
    const EVENT_TYPE: EventType = EventType::EntityRemoveEvent;
    type Data = EntityRemoveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityRemoveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityRemoveEvent(data)
    }
}
