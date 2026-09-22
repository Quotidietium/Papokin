use crate::wit::papokin::plugin::event::{Event, EventType, PlayerExpCooldownChangeEventData};

use super::super::FromIntoEvent;

/// 玩家经验冷却变化时触发的事件。
pub struct PlayerExpCooldownChangeEvent;
impl FromIntoEvent for PlayerExpCooldownChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerExpCooldownChangeEvent;
    type Data = PlayerExpCooldownChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerExpCooldownChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerExpCooldownChangeEvent(data)
    }
}
