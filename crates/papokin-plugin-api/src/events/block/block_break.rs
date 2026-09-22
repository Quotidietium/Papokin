use crate::wit::papokin::plugin::event::{BlockBreakEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块被破坏时触发的事件。
///
/// 关联的 [`BlockBreakEventData`] 包含玩家（如有）、方块
/// 标识符、其位置、要掉落的经验，以及该方块是否应当
/// 掉落物品。此事件可取消。
pub struct BlockBreakEvent;
impl FromIntoEvent for BlockBreakEvent {
    const EVENT_TYPE: EventType = EventType::BlockBreakEvent;
    type Data = BlockBreakEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockBreakEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockBreakEvent(data)
    }
}
