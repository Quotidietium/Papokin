use crate::wit::papokin::plugin::event::{ChunkSendEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 区块发送给客户端时触发的事件。
pub struct ChunkSendEvent;

impl FromIntoEvent for ChunkSendEvent {
    const EVENT_TYPE: EventType = EventType::ChunkSendEvent;
    type Data = ChunkSendEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ChunkSendEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ChunkSendEvent(data)
    }
}
