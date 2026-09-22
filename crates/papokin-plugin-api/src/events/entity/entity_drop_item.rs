use crate::wit::papokin::plugin::event::{EntityDropItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体掉落物品时触发的事件。
pub struct EntityDropItemEvent;
impl FromIntoEvent for EntityDropItemEvent {
    const EVENT_TYPE: EventType = EventType::EntityDropItemEvent;
    type Data = EntityDropItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDropItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDropItemEvent(data)
    }
}
