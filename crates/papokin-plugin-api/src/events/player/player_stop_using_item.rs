use crate::wit::papokin::plugin::event::{Event, EventType, PlayerStopUsingItemEventData};

use super::super::FromIntoEvent;

/// 玩家停止使用物品（如松开
/// 弓）。
pub struct PlayerStopUsingItemEvent;
impl FromIntoEvent for PlayerStopUsingItemEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStopUsingItemEvent;
    type Data = PlayerStopUsingItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStopUsingItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStopUsingItemEvent(data)
    }
}
