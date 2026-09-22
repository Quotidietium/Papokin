use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleCollisionEventData};

/// 载具碰撞时触发的基础事件。
pub struct VehicleCollisionEvent;
impl FromIntoEvent for VehicleCollisionEvent {
    const EVENT_TYPE: EventType = EventType::VehicleCollisionEvent;
    type Data = VehicleCollisionEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleCollisionEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleCollisionEvent(data)
    }
}
