use crate::wit::papokin::plugin::event::{ChunkSaveEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 区块在世界中保存时触发的事件。
pub struct ChunkSaveEvent;

impl FromIntoEvent for ChunkSaveEvent {
    const EVENT_TYPE: EventType = EventType::ChunkSaveEvent;
    type Data = ChunkSaveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ChunkSaveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ChunkSaveEvent(data)
    }
}
