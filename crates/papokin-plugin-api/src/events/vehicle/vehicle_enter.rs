use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleEnterEventData};

/// 实体进入载具时触发的事件。
pub struct VehicleEnterEvent;
impl FromIntoEvent for VehicleEnterEvent {
    const EVENT_TYPE: EventType = EventType::VehicleEnterEvent;
    type Data = VehicleEnterEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleEnterEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleEnterEvent(data)
    }
}
