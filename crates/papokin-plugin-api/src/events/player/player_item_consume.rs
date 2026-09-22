use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemConsumeEventData};

/// 玩家消耗物品时触发的事件。
pub struct PlayerItemConsumeEvent;
impl FromIntoEvent for PlayerItemConsumeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemConsumeEvent;
    type Data = PlayerItemConsumeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemConsumeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemConsumeEvent(data)
    }
}
