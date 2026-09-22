use crate::wit::papokin::plugin::event::{BeaconActivatedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 信标被激活（等级从 0 变为 n）时触发的事件。
pub struct BeaconActivatedEvent;
impl FromIntoEvent for BeaconActivatedEvent {
    const EVENT_TYPE: EventType = EventType::BeaconActivatedEvent;
    type Data = BeaconActivatedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BeaconActivatedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BeaconActivatedEvent(data)
    }
}
