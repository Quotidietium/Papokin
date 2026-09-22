use crate::wit::papokin::plugin::event::{EntitySpellCastEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 施法实体施放法术时触发的事件。
pub struct EntitySpellCastEvent;
impl FromIntoEvent for EntitySpellCastEvent {
    const EVENT_TYPE: EventType = EventType::EntitySpellCastEvent;
    type Data = EntitySpellCastEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntitySpellCastEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntitySpellCastEvent(data)
    }
}
