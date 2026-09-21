use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerJumpEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player jumps. This event is cancellable;
/// cancelling skips the jump statistic and exhaustion bookkeeping only.
pub struct PlayerJumpEvent;
impl FromIntoEvent for PlayerJumpEvent {
    const EVENT_TYPE: EventType = EventType::PlayerJumpEvent;
    type Data = PlayerJumpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerJumpEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerJumpEvent(data)
    }
}
