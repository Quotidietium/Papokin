use crate::wit::papokin::plugin::event::{Event, EventType, PlayerSwapHandsEventData};

use super::super::FromIntoEvent;

/// 玩家在双手间交换物品时触发的事件。
pub struct PlayerSwapHandsEvent;
impl FromIntoEvent for PlayerSwapHandsEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSwapHandsEvent;
    type Data = PlayerSwapHandsEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSwapHandsEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSwapHandsEvent(data)
    }
}
