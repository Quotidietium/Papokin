use crate::wit::papokin::plugin::event::{EntityLoadCrossbowEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体为弩装填弹射物时触发的事件。
pub struct EntityLoadCrossbowEvent;
impl FromIntoEvent for EntityLoadCrossbowEvent {
    const EVENT_TYPE: EventType = EventType::EntityLoadCrossbowEvent;
    type Data = EntityLoadCrossbowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityLoadCrossbowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityLoadCrossbowEvent(data)
    }
}
