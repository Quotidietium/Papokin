use crate::wit::papokin::plugin::event::{EntityCombustEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体着火时触发的事件。
pub struct EntityCombustEvent;
impl FromIntoEvent for EntityCombustEvent {
    const EVENT_TYPE: EventType = EventType::EntityCombustEvent;
    type Data = EntityCombustEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityCombustEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityCombustEvent(data)
    }
}
