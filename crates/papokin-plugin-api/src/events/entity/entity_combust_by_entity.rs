use crate::wit::papokin::plugin::event::{EntityCombustByEntityEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被另一实体点燃时触发的事件。
pub struct EntityCombustByEntityEvent;
impl FromIntoEvent for EntityCombustByEntityEvent {
    const EVENT_TYPE: EventType = EventType::EntityCombustByEntityEvent;
    type Data = EntityCombustByEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityCombustByEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityCombustByEntityEvent(data)
    }
}
