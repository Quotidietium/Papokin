use crate::wit::papokin::plugin::event::{Event, EventType, PigZapEventData};

use super::super::FromIntoEvent;

/// 猪因闪电变为僵尸猪人时触发的事件。
pub struct PigZapEvent;
impl FromIntoEvent for PigZapEvent {
    const EVENT_TYPE: EventType = EventType::PigZapEvent;
    type Data = PigZapEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PigZapEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PigZapEvent(data)
    }
}
