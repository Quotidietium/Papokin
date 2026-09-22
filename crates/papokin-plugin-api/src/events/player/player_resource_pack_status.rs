use crate::wit::papokin::plugin::event::{Event, EventType, PlayerResourcePackStatusEventData};

use super::super::FromIntoEvent;

/// 玩家响应资源包请求时触发的事件。
pub struct PlayerResourcePackStatusEvent;
impl FromIntoEvent for PlayerResourcePackStatusEvent {
    const EVENT_TYPE: EventType = EventType::PlayerResourcePackStatusEvent;
    type Data = PlayerResourcePackStatusEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerResourcePackStatusEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerResourcePackStatusEvent(data)
    }
}
