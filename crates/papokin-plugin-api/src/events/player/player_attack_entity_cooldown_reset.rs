use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerAttackEntityCooldownResetEventData,
};

use super::super::FromIntoEvent;

/// 玩家攻击冷却因攻击
/// 一个实体。此事件可取消。
pub struct PlayerAttackEntityCooldownResetEvent;
impl FromIntoEvent for PlayerAttackEntityCooldownResetEvent {
    const EVENT_TYPE: EventType = EventType::PlayerAttackEntityCooldownResetEvent;
    type Data = PlayerAttackEntityCooldownResetEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerAttackEntityCooldownResetEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerAttackEntityCooldownResetEvent(data)
    }
}
