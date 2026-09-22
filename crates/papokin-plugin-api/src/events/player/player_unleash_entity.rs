use crate::wit::papokin::plugin::event::{Event, EventType, PlayerUnleashEntityEventData};

use super::super::FromIntoEvent;

/// 玩家解开拴绳实体时触发的事件。
pub struct PlayerUnleashEntityEvent;
impl FromIntoEvent for PlayerUnleashEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerUnleashEntityEvent;
    type Data = PlayerUnleashEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerUnleashEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerUnleashEntityEvent(data)
    }
}
