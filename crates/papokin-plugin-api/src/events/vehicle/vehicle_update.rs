use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleUpdateEventData};

/// 载具逐刻更新时触发的事件。
pub struct VehicleUpdateEvent;
impl FromIntoEvent for VehicleUpdateEvent {
    const EVENT_TYPE: EventType = EventType::VehicleUpdateEvent;
    type Data = VehicleUpdateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleUpdateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleUpdateEvent(data)
    }
}
