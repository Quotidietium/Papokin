use crate::wit::papokin::plugin::event::{Event, EventType, WaterBottleSplashEventData};

use super::super::FromIntoEvent;

/// 水瓶溅射到实体时触发的事件。
pub struct WaterBottleSplashEvent;
impl FromIntoEvent for WaterBottleSplashEvent {
    const EVENT_TYPE: EventType = EventType::WaterBottleSplashEvent;
    type Data = WaterBottleSplashEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::WaterBottleSplashEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::WaterBottleSplashEvent(data)
    }
}
