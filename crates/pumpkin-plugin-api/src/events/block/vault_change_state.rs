use crate::wit::pumpkin::plugin::event::{Event, EventType, VaultChangeStateEventData};

use super::super::FromIntoEvent;

/// Event triggered when a vault block changes state.
pub struct VaultChangeStateEvent;
impl FromIntoEvent for VaultChangeStateEvent {
    const EVENT_TYPE: EventType = EventType::VaultChangeStateEvent;
    type Data = VaultChangeStateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VaultChangeStateEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VaultChangeStateEvent(data)
    }
}
