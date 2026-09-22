use crate::wit::papokin::plugin::event::{Event, EventType, PlayerCommandPreprocessEventData};

use super::super::FromIntoEvent;

/// 玩家命令执行前触发的事件。
pub struct PlayerCommandPreprocessEvent;
impl FromIntoEvent for PlayerCommandPreprocessEvent {
    const EVENT_TYPE: EventType = EventType::PlayerCommandPreprocessEvent;
    type Data = PlayerCommandPreprocessEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerCommandPreprocessEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerCommandPreprocessEvent(data)
    }
}
