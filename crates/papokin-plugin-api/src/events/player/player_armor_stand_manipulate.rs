use crate::wit::papokin::plugin::event::{Event, EventType, PlayerArmorStandManipulateEventData};

use super::super::FromIntoEvent;

/// 玩家操作盔甲架时触发的事件。
pub struct PlayerArmorStandManipulateEvent;
impl FromIntoEvent for PlayerArmorStandManipulateEvent {
    const EVENT_TYPE: EventType = EventType::PlayerArmorStandManipulateEvent;
    type Data = PlayerArmorStandManipulateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerArmorStandManipulateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerArmorStandManipulateEvent(data)
    }
}
