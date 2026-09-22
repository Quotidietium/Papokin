use crate::wit::papokin::plugin::event::{EntityAirChangeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体空气值变化时触发的事件。
pub struct EntityAirChangeEvent;
impl FromIntoEvent for EntityAirChangeEvent {
    const EVENT_TYPE: EventType = EventType::EntityAirChangeEvent;
    type Data = EntityAirChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityAirChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityAirChangeEvent(data)
    }
}
