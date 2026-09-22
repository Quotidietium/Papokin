use crate::wit::papokin::plugin::event::{Event, EventType, PotionSplashEventData};

use super::super::FromIntoEvent;

/// 药水溅射时触发的事件。
pub struct PotionSplashEvent;
impl FromIntoEvent for PotionSplashEvent {
    const EVENT_TYPE: EventType = EventType::PotionSplashEvent;
    type Data = PotionSplashEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PotionSplashEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PotionSplashEvent(data)
    }
}
