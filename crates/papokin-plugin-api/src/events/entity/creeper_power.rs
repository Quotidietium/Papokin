use crate::wit::papokin::plugin::event::{CreeperPowerEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 苦力怕被充能时触发的事件。
pub struct CreeperPowerEvent;
impl FromIntoEvent for CreeperPowerEvent {
    const EVENT_TYPE: EventType = EventType::CreeperPowerEvent;
    type Data = CreeperPowerEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CreeperPowerEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CreeperPowerEvent(data)
    }
}
