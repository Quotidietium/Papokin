use crate::wit::papokin::plugin::event::{Event, EventType, PlayerClientOptionsChangeEventData};

use super::super::FromIntoEvent;

/// 玩家客户端选项变化时触发的事件。这是一个
/// Java 版协议事件。
pub struct PlayerClientOptionsChangeEvent;
impl FromIntoEvent for PlayerClientOptionsChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerClientOptionsChangeEvent;
    type Data = PlayerClientOptionsChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerClientOptionsChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerClientOptionsChangeEvent(data)
    }
}
