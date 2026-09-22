use crate::wit::papokin::plugin::event::{EntityTargetBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体以方块为目标时触发的事件。
pub struct EntityTargetBlockEvent;
impl FromIntoEvent for EntityTargetBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityTargetBlockEvent;
    type Data = EntityTargetBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTargetBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTargetBlockEvent(data)
    }
}
