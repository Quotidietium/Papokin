use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleBlockCollisionEventData};

/// 载具与方块碰撞时触发的事件。
pub struct VehicleBlockCollisionEvent;
impl FromIntoEvent for VehicleBlockCollisionEvent {
    const EVENT_TYPE: EventType = EventType::VehicleBlockCollisionEvent;
    type Data = VehicleBlockCollisionEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleBlockCollisionEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleBlockCollisionEvent(data)
    }
}
