use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLinksSendEventData};

use super::super::FromIntoEvent;

/// 向玩家发送服务器链接时触发的事件。
pub struct PlayerLinksSendEvent;
impl FromIntoEvent for PlayerLinksSendEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLinksSendEvent;
    type Data = PlayerLinksSendEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLinksSendEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLinksSendEvent(data)
    }
}
