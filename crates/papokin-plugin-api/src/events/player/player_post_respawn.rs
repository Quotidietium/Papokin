use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPostRespawnEventData};

use super::super::FromIntoEvent;

/// 玩家重生之后触发的事件。
pub struct PlayerPostRespawnEvent;
impl FromIntoEvent for PlayerPostRespawnEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPostRespawnEvent;
    type Data = PlayerPostRespawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPostRespawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPostRespawnEvent(data)
    }
}
