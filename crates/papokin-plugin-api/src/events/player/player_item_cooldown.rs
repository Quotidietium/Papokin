use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemCooldownEventData};

use super::super::FromIntoEvent;

/// 某物品类型被应用冷却（对
/// 玩家。此事件可取消。
pub struct PlayerItemCooldownEvent;
impl FromIntoEvent for PlayerItemCooldownEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemCooldownEvent;
    type Data = PlayerItemCooldownEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemCooldownEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemCooldownEvent(data)
    }
}
