use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryPickupItemEventData};

/// 容器物品栏拾取物品实体时触发的事件。
pub struct InventoryPickupItemEvent;
impl FromIntoEvent for InventoryPickupItemEvent {
    const EVENT_TYPE: EventType = EventType::InventoryPickupItemEvent;
    type Data = InventoryPickupItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryPickupItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryPickupItemEvent(data)
    }
}
