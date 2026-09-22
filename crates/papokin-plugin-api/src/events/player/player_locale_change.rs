use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLocaleChangeEventData};

use super::super::FromIntoEvent;

/// 玩家更改客户端语言/区域设置时触发的事件。
pub struct PlayerLocaleChangeEvent;
impl FromIntoEvent for PlayerLocaleChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLocaleChangeEvent;
    type Data = PlayerLocaleChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLocaleChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLocaleChangeEvent(data)
    }
}
