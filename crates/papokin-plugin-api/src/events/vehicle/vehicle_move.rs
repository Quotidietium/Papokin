use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleMoveEventData};

/// 载具移动时触发的事件。
pub struct VehicleMoveEvent;
impl FromIntoEvent for VehicleMoveEvent {
    const EVENT_TYPE: EventType = EventType::VehicleMoveEvent;
    type Data = VehicleMoveEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleMoveEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleMoveEvent(data)
    }
}
