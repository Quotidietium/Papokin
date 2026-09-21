use crate::wit::pumpkin::plugin::event::{Event, EventType, WhitelistToggleEventData};

use super::super::FromIntoEvent;

/// Event triggered when the whitelist is toggled on or off.
pub struct WhitelistToggleEvent;
impl FromIntoEvent for WhitelistToggleEvent {
    const EVENT_TYPE: EventType = EventType::WhitelistToggleEvent;
    type Data = WhitelistToggleEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WhitelistToggleEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WhitelistToggleEvent(data)
    }
}
