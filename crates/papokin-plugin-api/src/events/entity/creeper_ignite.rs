use crate::wit::papokin::plugin::event::{CreeperIgniteEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 苦力怕被点燃时触发的事件。
pub struct CreeperIgniteEvent;
impl FromIntoEvent for CreeperIgniteEvent {
    const EVENT_TYPE: EventType = EventType::CreeperIgniteEvent;
    type Data = CreeperIgniteEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CreeperIgniteEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CreeperIgniteEvent(data)
    }
}
