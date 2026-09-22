use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerConnectionValidateLoginEventData,
};

use super::super::FromIntoEvent;

/// 验证入站登录时触发的事件，允许提前拒绝。
pub struct PlayerConnectionValidateLoginEvent;
impl FromIntoEvent for PlayerConnectionValidateLoginEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionValidateLoginEvent;
    type Data = PlayerConnectionValidateLoginEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionValidateLoginEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionValidateLoginEvent(data)
    }
}
