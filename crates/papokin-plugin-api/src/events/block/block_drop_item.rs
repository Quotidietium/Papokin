use crate::wit::papokin::plugin::event::{BlockDropItemEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块向世界掉落物品时触发的事件。
pub struct BlockDropItemEvent;
impl FromIntoEvent for BlockDropItemEvent {
    const EVENT_TYPE: EventType = EventType::BlockDropItemEvent;
    type Data = BlockDropItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockDropItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockDropItemEvent(data)
    }
}
