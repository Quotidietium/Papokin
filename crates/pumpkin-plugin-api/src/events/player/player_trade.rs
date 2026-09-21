use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerTradeEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player trades with a villager or wandering
/// trader. This event is cancellable.
pub struct PlayerTradeEvent;
impl FromIntoEvent for PlayerTradeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTradeEvent;
    type Data = PlayerTradeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTradeEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTradeEvent(data)
    }
}
