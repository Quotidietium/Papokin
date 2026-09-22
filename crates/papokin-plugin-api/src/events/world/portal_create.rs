use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PortalCreateEventData};

/// 传送门创建时触发的事件。
pub struct PortalCreateEvent;
impl FromIntoEvent for PortalCreateEvent {
    const EVENT_TYPE: EventType = EventType::PortalCreateEvent;
    type Data = PortalCreateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PortalCreateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PortalCreateEvent(data)
    }
}
