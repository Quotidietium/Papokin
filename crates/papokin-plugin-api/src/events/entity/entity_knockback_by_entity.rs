use crate::wit::papokin::plugin::event::{EntityKnockbackByEntityEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被另一实体击退时触发的事件。
pub struct EntityKnockbackByEntityEvent;
impl FromIntoEvent for EntityKnockbackByEntityEvent {
    const EVENT_TYPE: EventType = EventType::EntityKnockbackByEntityEvent;
    type Data = EntityKnockbackByEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityKnockbackByEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityKnockbackByEntityEvent(data)
    }
}
