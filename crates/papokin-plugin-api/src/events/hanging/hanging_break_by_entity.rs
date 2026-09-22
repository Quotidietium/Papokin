use crate::wit::papokin::plugin::event::{Event, EventType, HangingBreakByEntityEventData};

use super::super::FromIntoEvent;

/// 悬挂实体被另一实体破坏时触发的事件。
pub struct HangingBreakByEntityEvent;
impl FromIntoEvent for HangingBreakByEntityEvent {
    const EVENT_TYPE: EventType = EventType::HangingBreakByEntityEvent;
    type Data = HangingBreakByEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::HangingBreakByEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::HangingBreakByEntityEvent(data)
    }
}
