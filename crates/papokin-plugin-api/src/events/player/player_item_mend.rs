use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemMendEventData};

use super::super::FromIntoEvent;

/// 玩家物品被经验修补时触发的事件。
pub struct PlayerItemMendEvent;
impl FromIntoEvent for PlayerItemMendEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemMendEvent;
    type Data = PlayerItemMendEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemMendEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemMendEvent(data)
    }
}
