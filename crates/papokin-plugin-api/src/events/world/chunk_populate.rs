use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{ChunkPopulateEventData, Event, EventType};

/// 区块被填充时触发的事件。
pub struct ChunkPopulateEvent;
impl FromIntoEvent for ChunkPopulateEvent {
    const EVENT_TYPE: EventType = EventType::ChunkPopulateEvent;
    type Data = ChunkPopulateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ChunkPopulateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ChunkPopulateEvent(data)
    }
}
