use crate::wit::papokin::plugin::event::{EntityDismountEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体离开所骑乘实体时触发的事件。
pub struct EntityDismountEvent;
impl FromIntoEvent for EntityDismountEvent {
    const EVENT_TYPE: EventType = EventType::EntityDismountEvent;
    type Data = EntityDismountEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDismountEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDismountEvent(data)
    }
}
