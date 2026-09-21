use crate::wit::pumpkin::plugin::event::{BeaconDeactivatedEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when a beacon is deactivated (its tier drops to 0).
pub struct BeaconDeactivatedEvent;
impl FromIntoEvent for BeaconDeactivatedEvent {
    const EVENT_TYPE: EventType = EventType::BeaconDeactivatedEvent;
    type Data = BeaconDeactivatedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BeaconDeactivatedEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BeaconDeactivatedEvent(data)
    }
}
