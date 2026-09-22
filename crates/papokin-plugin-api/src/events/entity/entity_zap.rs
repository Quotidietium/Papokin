use crate::wit::papokin::plugin::event::{EntityZapEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体被闪电击中时触发的事件。
pub struct EntityZapEvent;
impl FromIntoEvent for EntityZapEvent {
    const EVENT_TYPE: EventType = EventType::EntityZapEvent;
    type Data = EntityZapEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityZapEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityZapEvent(data)
    }
}
