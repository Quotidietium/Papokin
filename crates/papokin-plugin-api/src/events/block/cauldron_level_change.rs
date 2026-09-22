use crate::wit::papokin::plugin::event::{CauldronLevelChangeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 炼药锅液位变化时触发的事件。
pub struct CauldronLevelChangeEvent;
impl FromIntoEvent for CauldronLevelChangeEvent {
    const EVENT_TYPE: EventType = EventType::CauldronLevelChangeEvent;
    type Data = CauldronLevelChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CauldronLevelChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CauldronLevelChangeEvent(data)
    }
}
