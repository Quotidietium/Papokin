use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemGroupCooldownEventData};

use super::super::FromIntoEvent;

/// 物品冷却组被应用冷却（对
/// 针对玩家。此事件可取消。
pub struct PlayerItemGroupCooldownEvent;
impl FromIntoEvent for PlayerItemGroupCooldownEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemGroupCooldownEvent;
    type Data = PlayerItemGroupCooldownEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemGroupCooldownEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemGroupCooldownEvent(data)
    }
}
