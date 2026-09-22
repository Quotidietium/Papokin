use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleExitEventData};

/// 实体离开载具时触发的事件。
pub struct VehicleExitEvent;
impl FromIntoEvent for VehicleExitEvent {
    const EVENT_TYPE: EventType = EventType::VehicleExitEvent;
    type Data = VehicleExitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleExitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleExitEvent(data)
    }
}
