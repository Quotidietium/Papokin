use crate::wit::papokin::plugin::event::{EntityBlockFormEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块因实体行为形成时触发的事件。
pub struct EntityBlockFormEvent;
impl FromIntoEvent for EntityBlockFormEvent {
    const EVENT_TYPE: EventType = EventType::EntityBlockFormEvent;
    type Data = EntityBlockFormEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityBlockFormEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityBlockFormEvent(data)
    }
}
