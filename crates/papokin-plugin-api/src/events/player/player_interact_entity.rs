use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInteractEntityEventData};

use super::super::FromIntoEvent;

/// 玩家与实体交互时触发的事件。
pub struct PlayerInteractEntityEvent;

impl FromIntoEvent for PlayerInteractEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInteractEntityEvent;
    type Data = PlayerInteractEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInteractEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInteractEntityEvent(data)
    }
}
