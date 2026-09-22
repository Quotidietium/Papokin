use crate::wit::papokin::plugin::event::{EntityDamageByEntityEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体受到另一实体伤害时触发的事件。
pub struct EntityDamageByEntityEvent;
impl FromIntoEvent for EntityDamageByEntityEvent {
    const EVENT_TYPE: EventType = EventType::EntityDamageByEntityEvent;
    type Data = EntityDamageByEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDamageByEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDamageByEntityEvent(data)
    }
}
