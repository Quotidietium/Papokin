use crate::wit::papokin::plugin::event::{AsyncPlayerSendSuggestionsEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 命令建议发送给
/// 玩家。此事件可取消；建议内容可被修改。
pub struct AsyncPlayerSendSuggestionsEvent;
impl FromIntoEvent for AsyncPlayerSendSuggestionsEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerSendSuggestionsEvent;
    type Data = AsyncPlayerSendSuggestionsEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerSendSuggestionsEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerSendSuggestionsEvent(data)
    }
}
