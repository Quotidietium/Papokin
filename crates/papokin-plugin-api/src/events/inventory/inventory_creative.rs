use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryCreativeEventData};

/// 创造模式玩家设置物品栏槽位时触发的事件。
pub struct InventoryCreativeEvent;
impl FromIntoEvent for InventoryCreativeEvent {
    const EVENT_TYPE: EventType = EventType::InventoryCreativeEvent;
    type Data = InventoryCreativeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryCreativeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryCreativeEvent(data)
    }
}
