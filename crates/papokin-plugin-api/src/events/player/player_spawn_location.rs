use crate::wit::papokin::plugin::event::{Event, EventType, PlayerSpawnLocationEventData};

use super::super::FromIntoEvent;

/// 确定玩家出生位置时触发的事件。
pub struct PlayerSpawnLocationEvent;
impl FromIntoEvent for PlayerSpawnLocationEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSpawnLocationEvent;
    type Data = PlayerSpawnLocationEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSpawnLocationEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSpawnLocationEvent(data)
    }
}
