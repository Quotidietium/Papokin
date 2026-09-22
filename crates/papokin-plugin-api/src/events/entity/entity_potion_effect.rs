use crate::wit::papokin::plugin::event::{EntityPotionEffectEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 药水状态效果施加到实体时触发的事件。
pub struct EntityPotionEffectEvent;
impl FromIntoEvent for EntityPotionEffectEvent {
    const EVENT_TYPE: EventType = EventType::EntityPotionEffectEvent;
    type Data = EntityPotionEffectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPotionEffectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPotionEffectEvent(data)
    }
}
