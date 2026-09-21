use crate::wit::pumpkin::plugin::event::{
    Event, EventType, PlayerConnectionValidateLoginEventData,
};

use super::super::FromIntoEvent;

/// Event triggered when an incoming login is validated, allowing it to be denied early.
pub struct PlayerConnectionValidateLoginEvent;
impl FromIntoEvent for PlayerConnectionValidateLoginEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionValidateLoginEvent;
    type Data = PlayerConnectionValidateLoginEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionValidateLoginEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionValidateLoginEvent(data)
    }
}
