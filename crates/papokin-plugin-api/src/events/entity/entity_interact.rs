use crate::wit::papokin::plugin::event::{EntityInteractEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体与方块交互时触发的事件。
pub struct EntityInteractEvent;
impl FromIntoEvent for EntityInteractEvent {
    const EVENT_TYPE: EventType = EventType::EntityInteractEvent;
    type Data = EntityInteractEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityInteractEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityInteractEvent(data)
    }
}
