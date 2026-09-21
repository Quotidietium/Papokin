use crate::wit::pumpkin::plugin::event::{AsyncTabCompleteEventData, Event, EventType};

use super::super::FromIntoEvent;

/// An event that occurs asynchronously when tab completions are computed.
/// This event is cancellable; the completions may be modified.
pub struct AsyncTabCompleteEvent;
impl FromIntoEvent for AsyncTabCompleteEvent {
    const EVENT_TYPE: EventType = EventType::AsyncTabCompleteEvent;
    type Data = AsyncTabCompleteEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncTabCompleteEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncTabCompleteEvent(data)
    }
}
