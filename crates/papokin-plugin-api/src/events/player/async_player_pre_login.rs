use crate::wit::papokin::plugin::event::{AsyncPlayerPreLoginEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家尝试预登录时触发的异步事件。
pub struct AsyncPlayerPreLoginEvent;
impl FromIntoEvent for AsyncPlayerPreLoginEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerPreLoginEvent;
    type Data = AsyncPlayerPreLoginEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerPreLoginEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerPreLoginEvent(data)
    }
}
