use crate::wit::papokin::plugin::event::{EntityEffectTickEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体上每个生效的药水效果每刻触发的事件。
pub struct EntityEffectTickEvent;
impl FromIntoEvent for EntityEffectTickEvent {
    const EVENT_TYPE: EventType = EventType::EntityEffectTickEvent;
    type Data = EntityEffectTickEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityEffectTickEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityEffectTickEvent(data)
    }
}
