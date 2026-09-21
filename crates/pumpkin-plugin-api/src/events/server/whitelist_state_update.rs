use crate::wit::pumpkin::plugin::event::{Event, EventType, WhitelistStateUpdateEventData};

use super::super::FromIntoEvent;

/// Event triggered when the whitelist state of a player is updated.
pub struct WhitelistStateUpdateEvent;
impl FromIntoEvent for WhitelistStateUpdateEvent {
    const EVENT_TYPE: EventType = EventType::WhitelistStateUpdateEvent;
    type Data = WhitelistStateUpdateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WhitelistStateUpdateEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WhitelistStateUpdateEvent(data)
    }
}
