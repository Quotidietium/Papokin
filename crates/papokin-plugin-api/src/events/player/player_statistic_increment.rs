use crate::wit::papokin::plugin::event::{Event, EventType, PlayerStatisticIncrementEventData};

use super::super::FromIntoEvent;

/// 玩家统计值增加时触发的事件。
pub struct PlayerStatisticIncrementEvent;
impl FromIntoEvent for PlayerStatisticIncrementEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStatisticIncrementEvent;
    type Data = PlayerStatisticIncrementEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStatisticIncrementEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStatisticIncrementEvent(data)
    }
}
