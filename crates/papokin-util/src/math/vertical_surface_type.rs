use serde::Deserialize;

/// 表示用于碰撞检测和放置逻辑的各种垂直表面类型。
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalSurfaceType {
    /// 朝上的表面（方块/空间的顶部），用于天花板碰撞与悬挂放置。
    Ceiling,
    /// 朝下的表面（方块/空间的底面），用于地面碰撞和站立放置。
    Floor,
}
