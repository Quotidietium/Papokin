use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemHeldEventData};

use super::super::FromIntoEvent;

/// 玩家更改选中的快捷栏槽位时触发的事件。
///
/// 关联的 [`PlayerItemHeldEventData`] 包含玩家、先前的槽位索引、
/// 以及新的槽位索引。此事件可取消。
pub struct PlayerItemHeldEvent;
impl FromIntoEvent for PlayerItemHeldEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemHeldEvent;
    type Data = PlayerItemHeldEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemHeldEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemHeldEvent(data)
    }
}
