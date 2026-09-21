use crate::wit::pumpkin::plugin::event::{
    Event, EventType, PlayerConnectionInitialConfigureEventData,
};

use super::super::FromIntoEvent;

/// Event triggered when a player connection is initially configured.
pub struct PlayerConnectionInitialConfigureEvent;
impl FromIntoEvent for PlayerConnectionInitialConfigureEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionInitialConfigureEvent;
    type Data = PlayerConnectionInitialConfigureEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionInitialConfigureEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionInitialConfigureEvent(data)
    }
}
