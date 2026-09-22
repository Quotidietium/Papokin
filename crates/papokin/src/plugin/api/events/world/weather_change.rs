use crate::world::World;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 世界中的天气（降雨）变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct WeatherChangeEvent {
    /// 天气正在变化的世界。
    pub world: Arc<World>,

    /// 新的天气状态（true = 下雨，false = 晴朗）。
    pub to_weather_state: bool,
}

impl WeatherChangeEvent {
    #[must_use]
    pub const fn new(world: Arc<World>, to_weather_state: bool) -> Self {
        Self {
            world,
            to_weather_state,
            cancelled: false,
        }
    }
}

/// 世界中的雷暴状态变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ThunderChangeEvent {
    /// 雷暴状态正在变化的世界。
    pub world: Arc<World>,

    /// 新的雷暴状态（true = 雷暴，false = 晴朗）。
    pub to_thunder_state: bool,
}

impl ThunderChangeEvent {
    #[must_use]
    pub const fn new(world: Arc<World>, to_thunder_state: bool) -> Self {
        Self {
            world,
            to_thunder_state,
            cancelled: false,
        }
    }
}
