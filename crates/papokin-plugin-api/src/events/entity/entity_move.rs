use crate::wit::papokin::plugin::event::{EntityMoveEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体移动时触发的事件。
pub struct EntityMoveEvent;
impl FromIntoEvent for EntityMoveEvent {
    const EVENT_TYPE: EventType = EventType::EntityMoveEvent;
    type Data = EntityMoveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityMoveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityMoveEvent(data)
    }
}
