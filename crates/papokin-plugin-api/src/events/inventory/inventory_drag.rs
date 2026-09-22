use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryDragEventData};

/// 物品在物品栏中被拖拽时触发的事件。
pub struct InventoryDragEvent;
impl FromIntoEvent for InventoryDragEvent {
    const EVENT_TYPE: EventType = EventType::InventoryDragEvent;
    type Data = InventoryDragEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryDragEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryDragEvent(data)
    }
}
