use crate::wit::papokin::plugin::event::{Event, EventType, PlayerVelocityEventData};

use super::super::FromIntoEvent;

/// 玩家速度变化时触发的事件。
pub struct PlayerVelocityEvent;
impl FromIntoEvent for PlayerVelocityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerVelocityEvent;
    type Data = PlayerVelocityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerVelocityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerVelocityEvent(data)
    }
}
