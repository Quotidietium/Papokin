use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleCreateEventData};

/// 载具创建时触发的事件。
pub struct VehicleCreateEvent;
impl FromIntoEvent for VehicleCreateEvent {
    const EVENT_TYPE: EventType = EventType::VehicleCreateEvent;
    type Data = VehicleCreateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleCreateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleCreateEvent(data)
    }
}
