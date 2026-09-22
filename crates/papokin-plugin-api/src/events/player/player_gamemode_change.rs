use crate::wit::papokin::plugin::event::{Event, EventType, PlayerGamemodeChangeEventData};

use super::super::FromIntoEvent;

/// 玩家游戏模式变化时触发的事件。
///
/// 关联的 [`PlayerGamemodeChangeEventData`] 包含玩家、先前的
/// 游戏模式以及新的游戏模式。此事件可取消。
pub struct PlayerGamemodeChangeEvent;
impl FromIntoEvent for PlayerGamemodeChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerGamemodeChangeEvent;
    type Data = PlayerGamemodeChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerGamemodeChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerGamemodeChangeEvent(data)
    }
}
