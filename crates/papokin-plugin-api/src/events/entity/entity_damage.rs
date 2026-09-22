use crate::wit::papokin::plugin::event::{EntityDamageEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体受到伤害时触发的事件。
pub struct EntityDamageEvent;
impl FromIntoEvent for EntityDamageEvent {
    const EVENT_TYPE: EventType = EventType::EntityDamageEvent;
    type Data = EntityDamageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDamageEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDamageEvent(data)
    }
}
