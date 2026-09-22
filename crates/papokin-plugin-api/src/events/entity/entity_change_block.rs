use crate::wit::papokin::plugin::event::{EntityChangeBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体改变世界中方块时触发的事件。
pub struct EntityChangeBlockEvent;
impl FromIntoEvent for EntityChangeBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityChangeBlockEvent;
    type Data = EntityChangeBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityChangeBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityChangeBlockEvent(data)
    }
}
