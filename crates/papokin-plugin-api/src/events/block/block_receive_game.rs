use crate::wit::papokin::plugin::event::{BlockReceiveGameEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块收到游戏事件时触发的事件。
pub struct BlockReceiveGameEvent;
impl FromIntoEvent for BlockReceiveGameEvent {
    const EVENT_TYPE: EventType = EventType::BlockReceiveGameEvent;
    type Data = BlockReceiveGameEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockReceiveGameEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockReceiveGameEvent(data)
    }
}
