use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLevelChangeEventData};

use super::super::FromIntoEvent;

/// 玩家等级变化时触发的事件。
pub struct PlayerLevelChangeEvent;
impl FromIntoEvent for PlayerLevelChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLevelChangeEvent;
    type Data = PlayerLevelChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLevelChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLevelChangeEvent(data)
    }
}
