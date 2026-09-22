use crate::wit::papokin::plugin::event::{Event, EventType, PlayerSpawnChangeEventData};

use super::super::FromIntoEvent;

/// 玩家重生位置变化时触发的事件。
pub struct PlayerSpawnChangeEvent;
impl FromIntoEvent for PlayerSpawnChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSpawnChangeEvent;
    type Data = PlayerSpawnChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSpawnChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSpawnChangeEvent(data)
    }
}
