use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChunkUnloadEventData};

use super::super::FromIntoEvent;

/// 区块在玩家客户端卸载时触发的事件。
pub struct PlayerChunkUnloadEvent;
impl FromIntoEvent for PlayerChunkUnloadEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChunkUnloadEvent;
    type Data = PlayerChunkUnloadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChunkUnloadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChunkUnloadEvent(data)
    }
}
