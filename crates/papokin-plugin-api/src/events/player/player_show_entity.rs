use crate::wit::papokin::plugin::event::{Event, EventType, PlayerShowEntityEventData};

use super::super::FromIntoEvent;

/// 实体对玩家可见时触发的事件。
pub struct PlayerShowEntityEvent;
impl FromIntoEvent for PlayerShowEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerShowEntityEvent;
    type Data = PlayerShowEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerShowEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerShowEntityEvent(data)
    }
}
