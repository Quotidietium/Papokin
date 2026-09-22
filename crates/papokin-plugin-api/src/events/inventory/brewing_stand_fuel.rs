use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BrewingStandFuelEventData, Event, EventType};

/// 酿造台燃料被消耗或补充时触发的事件。
pub struct BrewingStandFuelEvent;
impl FromIntoEvent for BrewingStandFuelEvent {
    const EVENT_TYPE: EventType = EventType::BrewingStandFuelEvent;
    type Data = BrewingStandFuelEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BrewingStandFuelEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BrewingStandFuelEvent(data)
    }
}
