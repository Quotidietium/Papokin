use crate::wit::papokin::plugin::event::{EntityEnterBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体进入方块时触发的事件。
pub struct EntityEnterBlockEvent;
impl FromIntoEvent for EntityEnterBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityEnterBlockEvent;
    type Data = EntityEnterBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityEnterBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityEnterBlockEvent(data)
    }
}
