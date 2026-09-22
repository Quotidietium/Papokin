use crate::wit::papokin::plugin::event::{Event, EventType, ExplosionPrimeEventData};

use super::super::FromIntoEvent;

/// 实体被点燃待爆时触发的事件。
pub struct ExplosionPrimeEvent;
impl FromIntoEvent for ExplosionPrimeEvent {
    const EVENT_TYPE: EventType = EventType::ExplosionPrimeEvent;
    type Data = ExplosionPrimeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ExplosionPrimeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ExplosionPrimeEvent(data)
    }
}
