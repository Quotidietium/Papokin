use crate::wit::pumpkin::plugin::event::{Event, EventType, ProfileWhitelistVerifyEventData};

use super::super::FromIntoEvent;

/// Event triggered when a connecting player is verified against the whitelist.
pub struct ProfileWhitelistVerifyEvent;
impl FromIntoEvent for ProfileWhitelistVerifyEvent {
    const EVENT_TYPE: EventType = EventType::ProfileWhitelistVerifyEvent;
    type Data = ProfileWhitelistVerifyEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ProfileWhitelistVerifyEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ProfileWhitelistVerifyEvent(data)
    }
}
