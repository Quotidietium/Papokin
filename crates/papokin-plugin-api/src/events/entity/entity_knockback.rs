use crate::wit::papokin::plugin::event::{EntityKnockbackEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体受到击退时触发的事件。
pub struct EntityKnockbackEvent;
impl FromIntoEvent for EntityKnockbackEvent {
    const EVENT_TYPE: EventType = EventType::EntityKnockbackEvent;
    type Data = EntityKnockbackEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityKnockbackEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityKnockbackEvent(data)
    }
}
