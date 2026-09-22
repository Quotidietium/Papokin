use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPreLoginEventData};

use super::super::FromIntoEvent;

/// 玩家预登录时同步触发的事件。
pub struct PlayerPreLoginEvent;
impl FromIntoEvent for PlayerPreLoginEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPreLoginEvent;
    type Data = PlayerPreLoginEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPreLoginEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPreLoginEvent(data)
    }
}
