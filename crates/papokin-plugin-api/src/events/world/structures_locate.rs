use crate::wit::papokin::plugin::event::{Event, EventType, StructuresLocateEventData};

use super::super::FromIntoEvent;

/// 服务器定位结构（如 locate 命令）时触发的事件。
pub struct StructuresLocateEvent;
impl FromIntoEvent for StructuresLocateEvent {
    const EVENT_TYPE: EventType = EventType::StructuresLocateEvent;
    type Data = StructuresLocateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::StructuresLocateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::StructuresLocateEvent(data)
    }
}
