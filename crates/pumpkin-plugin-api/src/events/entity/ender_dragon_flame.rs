use crate::wit::pumpkin::plugin::event::{EnderDragonFlameEventData, Event, EventType};

use super::super::FromIntoEvent;

/// Event triggered when an ender dragon breathes fire.
pub struct EnderDragonFlameEvent;
impl FromIntoEvent for EnderDragonFlameEvent {
    const EVENT_TYPE: EventType = EventType::EnderDragonFlameEvent;
    type Data = EnderDragonFlameEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EnderDragonFlameEvent(data) => data,
            _ => panic!("unexpected event"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EnderDragonFlameEvent(data)
    }
}
