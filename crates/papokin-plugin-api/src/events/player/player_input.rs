use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInputEventData};

use super::super::FromIntoEvent;

/// 收到玩家输入时触发的事件。
pub struct PlayerInputEvent;
impl FromIntoEvent for PlayerInputEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInputEvent;
    type Data = PlayerInputEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInputEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInputEvent(data)
    }
}
