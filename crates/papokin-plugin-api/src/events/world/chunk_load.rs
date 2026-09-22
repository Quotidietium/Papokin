use crate::wit::papokin::plugin::event::{ChunkLoadEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 区块在世界中加载时触发的事件。
pub struct ChunkLoadEvent;

impl FromIntoEvent for ChunkLoadEvent {
    const EVENT_TYPE: EventType = EventType::ChunkLoadEvent;
    type Data = ChunkLoadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ChunkLoadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ChunkLoadEvent(data)
    }
}
