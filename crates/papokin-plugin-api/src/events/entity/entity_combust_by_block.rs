use crate::wit::papokin::plugin::event::{EntityCombustByBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被方块点燃时触发的事件。
pub struct EntityCombustByBlockEvent;
impl FromIntoEvent for EntityCombustByBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityCombustByBlockEvent;
    type Data = EntityCombustByBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityCombustByBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityCombustByBlockEvent(data)
    }
}
