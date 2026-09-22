use crate::wit::papokin::plugin::event::{Event, EventType, PlayerAnimationEventData};

use super::super::FromIntoEvent;

/// 玩家播放动作动画时触发的事件。
pub struct PlayerAnimationEvent;
impl FromIntoEvent for PlayerAnimationEvent {
    const EVENT_TYPE: EventType = EventType::PlayerAnimationEvent;
    type Data = PlayerAnimationEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerAnimationEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerAnimationEvent(data)
    }
}
