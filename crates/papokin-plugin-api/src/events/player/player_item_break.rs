use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemBreakEventData};

use super::super::FromIntoEvent;

/// 玩家损坏物品时触发的事件。
pub struct PlayerItemBreakEvent;
impl FromIntoEvent for PlayerItemBreakEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemBreakEvent;
    type Data = PlayerItemBreakEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemBreakEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemBreakEvent(data)
    }
}
