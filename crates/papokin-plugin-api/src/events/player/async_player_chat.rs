use crate::wit::papokin::plugin::event::{AsyncPlayerChatEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家发送聊天消息时触发的异步事件。
pub struct AsyncPlayerChatEvent;
impl FromIntoEvent for AsyncPlayerChatEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerChatEvent;
    type Data = AsyncPlayerChatEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerChatEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerChatEvent(data)
    }
}
