use crate::wit::papokin::plugin::event::{Event, EventType, PlayerDeepSleepEventData};

use super::super::FromIntoEvent;

/// 玩家进入深度睡眠（睡眠 100 刻）时触发的事件。
pub struct PlayerDeepSleepEvent;
impl FromIntoEvent for PlayerDeepSleepEvent {
    const EVENT_TYPE: EventType = EventType::PlayerDeepSleepEvent;
    type Data = PlayerDeepSleepEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerDeepSleepEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerDeepSleepEvent(data)
    }
}
