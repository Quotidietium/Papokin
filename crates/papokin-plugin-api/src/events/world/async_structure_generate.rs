use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{AsyncStructureGenerateEventData, Event, EventType};

/// 结构异步生成时触发的事件。
pub struct AsyncStructureGenerateEvent;
impl FromIntoEvent for AsyncStructureGenerateEvent {
    const EVENT_TYPE: EventType = EventType::AsyncStructureGenerateEvent;
    type Data = AsyncStructureGenerateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncStructureGenerateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncStructureGenerateEvent(data)
    }
}
