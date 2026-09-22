use crate::wit::papokin::plugin::event::{EnchantItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 物品在附魔台附魔时触发的事件。
pub struct EnchantItemEvent;
impl FromIntoEvent for EnchantItemEvent {
    const EVENT_TYPE: EventType = EventType::EnchantItemEvent;
    type Data = EnchantItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EnchantItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EnchantItemEvent(data)
    }
}
