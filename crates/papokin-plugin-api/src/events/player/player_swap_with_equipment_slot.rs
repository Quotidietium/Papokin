use crate::wit::papokin::plugin::event::{Event, EventType, PlayerSwapWithEquipmentSlotEventData};

use super::super::FromIntoEvent;

/// 玩家与装备槽交换物品时触发的事件。
/// 此事件可取消。
pub struct PlayerSwapWithEquipmentSlotEvent;
impl FromIntoEvent for PlayerSwapWithEquipmentSlotEvent {
    const EVENT_TYPE: EventType = EventType::PlayerSwapWithEquipmentSlotEvent;
    type Data = PlayerSwapWithEquipmentSlotEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerSwapWithEquipmentSlotEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerSwapWithEquipmentSlotEvent(data)
    }
}
