use crate::wit::papokin::plugin::event::{EntityFertilizeEggEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体（海龟）为蛋受精时触发的事件。
pub struct EntityFertilizeEggEvent;
impl FromIntoEvent for EntityFertilizeEggEvent {
    const EVENT_TYPE: EventType = EventType::EntityFertilizeEggEvent;
    type Data = EntityFertilizeEggEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityFertilizeEggEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityFertilizeEggEvent(data)
    }
}
