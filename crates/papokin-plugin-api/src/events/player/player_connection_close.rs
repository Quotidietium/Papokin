use crate::wit::papokin::plugin::event::{Event, EventType, PlayerConnectionCloseEventData};

use super::super::FromIntoEvent;

/// 玩家连接关闭时触发的事件。
pub struct PlayerConnectionCloseEvent;
impl FromIntoEvent for PlayerConnectionCloseEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionCloseEvent;
    type Data = PlayerConnectionCloseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionCloseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionCloseEvent(data)
    }
}
