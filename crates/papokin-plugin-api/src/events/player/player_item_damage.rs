use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemDamageEventData};

/// 玩家手持物品受到损耗时触发的事件。
pub struct PlayerItemDamageEvent;
impl FromIntoEvent for PlayerItemDamageEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemDamageEvent;
    type Data = PlayerItemDamageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemDamageEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemDamageEvent(data)
    }
}
