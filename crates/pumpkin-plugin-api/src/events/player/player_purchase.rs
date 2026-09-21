use crate::wit::pumpkin::plugin::event::{Event, EventType, PlayerPurchaseEventData};

use super::super::FromIntoEvent;

/// An event that occurs when a player completes a purchase with a merchant.
/// This event is cancellable.
pub struct PlayerPurchaseEvent;
impl FromIntoEvent for PlayerPurchaseEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPurchaseEvent;
    type Data = PlayerPurchaseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPurchaseEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPurchaseEvent(data)
    }
}
