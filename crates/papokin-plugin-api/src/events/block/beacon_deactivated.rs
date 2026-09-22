use crate::wit::papokin::plugin::event::{BeaconDeactivatedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 信标被停用（等级降为 0）时触发的事件。
pub struct BeaconDeactivatedEvent;
impl FromIntoEvent for BeaconDeactivatedEvent {
    const EVENT_TYPE: EventType = EventType::BeaconDeactivatedEvent;
    type Data = BeaconDeactivatedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BeaconDeactivatedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BeaconDeactivatedEvent(data)
    }
}
