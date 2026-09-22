use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PlayerDropItemEventData};

/// 玩家丢出物品时触发的事件。
pub struct PlayerDropItemEvent;
impl FromIntoEvent for PlayerDropItemEvent {
    const EVENT_TYPE: EventType = EventType::PlayerDropItemEvent;
    type Data = PlayerDropItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerDropItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerDropItemEvent(data)
    }
}
