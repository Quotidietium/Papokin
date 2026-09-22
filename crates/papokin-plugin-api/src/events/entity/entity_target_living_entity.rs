use crate::wit::papokin::plugin::event::{EntityTargetLivingEntityEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体以生物实体为目标时触发的事件。
pub struct EntityTargetLivingEntityEvent;
impl FromIntoEvent for EntityTargetLivingEntityEvent {
    const EVENT_TYPE: EventType = EventType::EntityTargetLivingEntityEvent;
    type Data = EntityTargetLivingEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityTargetLivingEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityTargetLivingEntityEvent(data)
    }
}
