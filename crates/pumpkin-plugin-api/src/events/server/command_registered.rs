use crate::wit::pumpkin::plugin::event::{CommandRegisteredEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when a command is registered to the server.
pub struct CommandRegisteredEvent;
impl FromIntoEvent for CommandRegisteredEvent {
    const EVENT_TYPE: EventType = EventType::CommandRegisteredEvent;
    type Data = CommandRegisteredEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CommandRegisteredEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CommandRegisteredEvent(data)
    }
}
