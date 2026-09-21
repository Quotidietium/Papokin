use crate::wit::pumpkin::plugin::event::{AsyncPlayerSendSuggestionsEventData, Event, EventType};

use super::super::FromIntoEvent;

/// An event that occurs asynchronously when command suggestions are sent to a
/// player. This event is cancellable; the suggestions may be modified.
pub struct AsyncPlayerSendSuggestionsEvent;
impl FromIntoEvent for AsyncPlayerSendSuggestionsEvent {
    const EVENT_TYPE: EventType = EventType::AsyncPlayerSendSuggestionsEvent;
    type Data = AsyncPlayerSendSuggestionsEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncPlayerSendSuggestionsEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncPlayerSendSuggestionsEvent(data)
    }
}
