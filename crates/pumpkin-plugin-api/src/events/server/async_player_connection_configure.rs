use crate::wit::pumpkin::plugin::event::{
    AsyncPlayerConnectionConfigureEventData, Event, EventType,
};

use super::super::FromIntoEvent;

/// Event triggered asynchronously when a player connection is being configured.
pub struct AsyncPlayerConnectionConfigureEvent;
impl FromIntoEvent for AsyncPlayerConnectionConfigureEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerConnectionConfigureEvent;
    type Data = AsyncPlayerConnectionConfigureEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerConnectionConfigureEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerConnectionConfigureEvent(data)
    }
}
