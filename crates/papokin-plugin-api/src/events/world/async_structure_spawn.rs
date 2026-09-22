use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{AsyncStructureSpawnEventData, Event, EventType};

/// 结构异步放置时触发的事件。
pub struct AsyncStructureSpawnEvent;
impl FromIntoEvent for AsyncStructureSpawnEvent {
    const EVENT_TYPE: EventType = EventType::AsyncStructureSpawnEvent;
    type Data = AsyncStructureSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncStructureSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncStructureSpawnEvent(data)
    }
}
