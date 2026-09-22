use crate::wit::papokin::plugin::event::{Event, EventType, PlayerFishEventData};

use super::super::FromIntoEvent;

/// 钓鱼动作期间触发的事件。
///
/// 关联的 [`PlayerFishEventData`] 包含玩家、钓鱼状态、
/// 所用的手、鱼钩实体、可选的上钩实体，以及经验
/// 要掉落的物品。此事件可取消。
pub struct PlayerFishEvent;
impl FromIntoEvent for PlayerFishEvent {
    const EVENT_TYPE: EventType = EventType::PlayerFishEvent;
    type Data = PlayerFishEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerFishEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerFishEvent(data)
    }
}
