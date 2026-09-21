use crate::wit::pumpkin::plugin::event::{
    Event, EventType, ItemTransportingEntityValidateTargetEventData,
};

use super::super::FromIntoEvent;

/// Event triggered when an item-transporting entity validates its target block.
pub struct ItemTransportingEntityValidateTargetEvent;
impl FromIntoEvent for ItemTransportingEntityValidateTargetEvent {
    const EVENT_TYPE: EventType = EventType::ItemTransportingEntityValidateTargetEvent;
    type Data = ItemTransportingEntityValidateTargetEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ItemTransportingEntityValidateTargetEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ItemTransportingEntityValidateTargetEvent(data)
    }
}
