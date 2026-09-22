use crate::wit::papokin::plugin::event::{EntityDyeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被染色时触发的事件。
pub struct EntityDyeEvent;
impl FromIntoEvent for EntityDyeEvent {
    const EVENT_TYPE: EventType = EventType::EntityDyeEvent;
    type Data = EntityDyeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDyeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDyeEvent(data)
    }
}
