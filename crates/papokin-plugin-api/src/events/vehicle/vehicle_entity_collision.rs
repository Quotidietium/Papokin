use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleEntityCollisionEventData};

/// 载具与另一实体碰撞时触发的事件。
pub struct VehicleEntityCollisionEvent;
impl FromIntoEvent for VehicleEntityCollisionEvent {
    const EVENT_TYPE: EventType = EventType::VehicleEntityCollisionEvent;
    type Data = VehicleEntityCollisionEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleEntityCollisionEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleEntityCollisionEvent(data)
    }
}
