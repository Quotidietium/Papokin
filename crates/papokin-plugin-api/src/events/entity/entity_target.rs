use crate::wit::papokin::plugin::event::{EntityTargetEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体以另一实体为目标时触发的事件。
pub struct EntityTargetEvent;
impl FromIntoEvent for EntityTargetEvent {
    const EVENT_TYPE: EventType = EventType::EntityTargetEvent;
    type Data = EntityTargetEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTargetEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTargetEvent(data)
    }
}
