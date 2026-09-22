use crate::wit::papokin::plugin::event::{EntityJumpEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体跳跃时触发的事件。
pub struct EntityJumpEvent;
impl FromIntoEvent for EntityJumpEvent {
    const EVENT_TYPE: EventType = EventType::EntityJumpEvent;
    type Data = EntityJumpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityJumpEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityJumpEvent(data)
    }
}
