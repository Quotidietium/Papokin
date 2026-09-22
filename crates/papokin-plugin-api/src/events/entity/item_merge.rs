use crate::wit::papokin::plugin::event::{Event, EventType, ItemMergeEventData};

use super::super::FromIntoEvent;

/// 两个物品实体合并时触发的事件。
pub struct ItemMergeEvent;
impl FromIntoEvent for ItemMergeEvent {
    const EVENT_TYPE: EventType = EventType::ItemMergeEvent;
    type Data = ItemMergeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ItemMergeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ItemMergeEvent(data)
    }
}
