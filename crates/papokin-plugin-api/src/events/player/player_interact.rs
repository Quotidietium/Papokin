use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInteractEventData};

use super::super::FromIntoEvent;

/// 玩家交互时触发的事件。
pub struct PlayerInteractEvent;
impl FromIntoEvent for PlayerInteractEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInteractEvent;
    type Data = PlayerInteractEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInteractEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInteractEvent(data)
    }
}
