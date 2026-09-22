use crate::wit::papokin::plugin::event::{EntityPickupItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体拾取物品时触发的事件。
pub struct EntityPickupItemEvent;
impl FromIntoEvent for EntityPickupItemEvent {
    const EVENT_TYPE: EventType = EventType::EntityPickupItemEvent;
    type Data = EntityPickupItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPickupItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPickupItemEvent(data)
    }
}
