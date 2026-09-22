use crate::wit::papokin::plugin::event::{Event, EventType, PrepareItemEnchantEventData};

use super::super::FromIntoEvent;

/// 物品被放入附魔台以准备附魔选项时触发的事件。
pub struct PrepareItemEnchantEvent;
impl FromIntoEvent for PrepareItemEnchantEvent {
    const EVENT_TYPE: EventType = EventType::PrepareItemEnchantEvent;
    type Data = PrepareItemEnchantEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareItemEnchantEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareItemEnchantEvent(data)
    }
}
