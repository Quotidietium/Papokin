use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, VehicleDamageEventData};

/// 载具受到伤害时触发的事件。
pub struct VehicleDamageEvent;
impl FromIntoEvent for VehicleDamageEvent {
    const EVENT_TYPE: EventType = EventType::VehicleDamageEvent;
    type Data = VehicleDamageEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VehicleDamageEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VehicleDamageEvent(data)
    }
}
