use crate::wit::papokin::plugin::event::{EntityRegainHealthEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体恢复生命时触发的事件。
pub struct EntityRegainHealthEvent;
impl FromIntoEvent for EntityRegainHealthEvent {
    const EVENT_TYPE: EventType = EventType::EntityRegainHealthEvent;
    type Data = EntityRegainHealthEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityRegainHealthEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityRegainHealthEvent(data)
    }
}
