use crate::wit::papokin::plugin::event::{EntityDamageByBlockEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体受到方块伤害时触发的事件。
pub struct EntityDamageByBlockEvent;
impl FromIntoEvent for EntityDamageByBlockEvent {
    const EVENT_TYPE: EventType = EventType::EntityDamageByBlockEvent;
    type Data = EntityDamageByBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDamageByBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDamageByBlockEvent(data)
    }
}
