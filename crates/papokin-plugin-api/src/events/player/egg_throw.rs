use crate::wit::papokin::plugin::event::{Event, EventType, PlayerEggThrowEventData};

use super::super::FromIntoEvent;

/// 掷出的鸡蛋结算时触发的事件。
///
/// 关联的 [`PlayerEggThrowEventData`] 包含玩家、鸡蛋实体 UUID、
/// 蛋是否孵化、孵化出多少实体，以及要孵化的实体类型。
/// 此事件可取消。
pub struct PlayerEggThrowEvent;
impl FromIntoEvent for PlayerEggThrowEvent {
    const EVENT_TYPE: EventType = EventType::PlayerEggThrowEvent;
    type Data = PlayerEggThrowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerEggThrowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerEggThrowEvent(data)
    }
}
