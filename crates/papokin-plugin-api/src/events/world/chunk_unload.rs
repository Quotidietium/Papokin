use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{ChunkUnloadEventData, Event, EventType};

/// 区块卸载时触发的事件。
pub struct ChunkUnloadEvent;
impl FromIntoEvent for ChunkUnloadEvent {
    const EVENT_TYPE: EventType = EventType::ChunkUnloadEvent;
    type Data = ChunkUnloadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ChunkUnloadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ChunkUnloadEvent(data)
    }
}
