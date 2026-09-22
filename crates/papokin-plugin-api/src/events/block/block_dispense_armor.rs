use crate::wit::papokin::plugin::event::{BlockDispenseArmorEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 发射器为实体装备盔甲时触发的事件。
pub struct BlockDispenseArmorEvent;
impl FromIntoEvent for BlockDispenseArmorEvent {
    const EVENT_TYPE: EventType = EventType::BlockDispenseArmorEvent;
    type Data = BlockDispenseArmorEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDispenseArmorEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDispenseArmorEvent(data)
    }
}
