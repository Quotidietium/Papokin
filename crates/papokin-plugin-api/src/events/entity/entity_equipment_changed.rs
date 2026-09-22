use crate::wit::papokin::plugin::event::{EntityEquipmentChangedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体某槽位装备变化时触发的事件。
pub struct EntityEquipmentChangedEvent;
impl FromIntoEvent for EntityEquipmentChangedEvent {
    const EVENT_TYPE: EventType = EventType::EntityEquipmentChangedEvent;
    type Data = EntityEquipmentChangedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityEquipmentChangedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityEquipmentChangedEvent(data)
    }
}
