use crate::wit::pumpkin::plugin::event::{BeaconActivatedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when a beacon is activated (its tier goes from 0 to n).
pub struct BeaconActivatedEvent;
impl FromIntoEvent for BeaconActivatedEvent {
    const EVENT_TYPE: EventType = EventType::BeaconActivatedEvent;
    type Data = BeaconActivatedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BeaconActivatedEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BeaconActivatedEvent(data)
    }
}
