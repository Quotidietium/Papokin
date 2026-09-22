use crate::wit::papokin::plugin::event::{EntityShootBowEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体射箭时触发的事件。
pub struct EntityShootBowEvent;
impl FromIntoEvent for EntityShootBowEvent {
    const EVENT_TYPE: EventType = EventType::EntityShootBowEvent;
    type Data = EntityShootBowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityShootBowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityShootBowEvent(data)
    }
}
