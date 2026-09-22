use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockPhysicsEventData, Event, EventType};

/// 执行方块物理检查时触发的事件。
pub struct BlockPhysicsEvent;
impl FromIntoEvent for BlockPhysicsEvent {
    const EVENT_TYPE: EventType = EventType::BlockPhysicsEvent;
    type Data = BlockPhysicsEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockPhysicsEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockPhysicsEvent(data)
    }
}
