use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleDestroyEventData};

/// 载具被摧毁时触发的事件。
pub struct VehicleDestroyEvent;
impl FromIntoEvent for VehicleDestroyEvent {
    const EVENT_TYPE: EventType = EventType::VehicleDestroyEvent;
    type Data = VehicleDestroyEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleDestroyEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleDestroyEvent(data)
    }
}
