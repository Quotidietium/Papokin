use crate::wit::papokin::plugin::event::{Event, EventType, PlayerArmorChangeEventData};

use super::super::FromIntoEvent;

/// 玩家的盔甲部件变化时触发的事件。
pub struct PlayerArmorChangeEvent;
impl FromIntoEvent for PlayerArmorChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerArmorChangeEvent;
    type Data = PlayerArmorChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerArmorChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerArmorChangeEvent(data)
    }
}
