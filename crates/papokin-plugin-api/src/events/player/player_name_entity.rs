use crate::wit::papokin::plugin::event::{Event, EventType, PlayerNameEntityEventData};

use super::super::FromIntoEvent;

/// 玩家用命名牌为实体命名时触发的事件。
pub struct PlayerNameEntityEvent;
impl FromIntoEvent for PlayerNameEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerNameEntityEvent;
    type Data = PlayerNameEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerNameEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerNameEntityEvent(data)
    }
}
