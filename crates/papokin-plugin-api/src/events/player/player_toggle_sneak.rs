use crate::wit::papokin::plugin::event::{Event, EventType, PlayerToggleSneakEventData};

use super::super::FromIntoEvent;

/// 玩家切换潜行时触发的事件。
pub struct PlayerToggleSneakEvent;
impl FromIntoEvent for PlayerToggleSneakEvent {
    const EVENT_TYPE: EventType = EventType::PlayerToggleSneakEvent;
    type Data = PlayerToggleSneakEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerToggleSneakEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerToggleSneakEvent(data)
    }
}
