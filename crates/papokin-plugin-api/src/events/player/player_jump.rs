use crate::wit::papokin::plugin::event::{Event, EventType, PlayerJumpEventData};

use super::super::FromIntoEvent;

/// 玩家跳跃时触发的事件。此事件可取消；
/// 取消只会跳过跳跃统计与消耗值的记账。
pub struct PlayerJumpEvent;
impl FromIntoEvent for PlayerJumpEvent {
    const EVENT_TYPE: EventType = EventType::PlayerJumpEvent;
    type Data = PlayerJumpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerJumpEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerJumpEvent(data)
    }
}
