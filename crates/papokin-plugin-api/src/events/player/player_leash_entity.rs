use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLeashEntityEventData};

use super::super::FromIntoEvent;

/// 玩家拴住实体时触发的事件。
pub struct PlayerLeashEntityEvent;
impl FromIntoEvent for PlayerLeashEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLeashEntityEvent;
    type Data = PlayerLeashEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLeashEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLeashEntityEvent(data)
    }
}
