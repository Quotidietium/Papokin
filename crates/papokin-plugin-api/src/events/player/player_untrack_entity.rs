use crate::wit::papokin::plugin::event::{Event, EventType, PlayerUntrackEntityEventData};

use super::super::FromIntoEvent;

/// 实体停止被玩家追踪时触发的事件。
pub struct PlayerUntrackEntityEvent;
impl FromIntoEvent for PlayerUntrackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerUntrackEntityEvent;
    type Data = PlayerUntrackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerUntrackEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerUntrackEntityEvent(data)
    }
}
