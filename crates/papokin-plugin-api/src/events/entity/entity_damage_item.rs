use crate::wit::papokin::plugin::event::{EntityDamageItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体持有或穿戴的物品受到耐久损耗时触发的事件。
pub struct EntityDamageItemEvent;
impl FromIntoEvent for EntityDamageItemEvent {
    const EVENT_TYPE: EventType = EventType::EntityDamageItemEvent;
    type Data = EntityDamageItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDamageItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDamageItemEvent(data)
    }
}
