use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInteractAtEntityEventData};

use super::super::FromIntoEvent;

/// 玩家与实体上特定位置交互时触发的事件。
pub struct PlayerInteractAtEntityEvent;
impl FromIntoEvent for PlayerInteractAtEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInteractAtEntityEvent;
    type Data = PlayerInteractAtEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInteractAtEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInteractAtEntityEvent(data)
    }
}
