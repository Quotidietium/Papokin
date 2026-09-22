//! 方块旋转与镜像变换。
//!
//! 这些变换用于旋转和镜像方块
//! 在世界中放置它们时同样会应用，与原版 Minecraft 行为一致。

use crate::BlockDirection;
use crate::block_properties::HorizontalFacing;
use papokin_util::math::vector3::Vector3;
use serde::Deserialize;

/// 绕 Y 轴的旋转，以 90 度为增量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// 不旋转（0 度）
    #[default]
    None,
    /// 顺时针 90 度
    Clockwise90,
    /// 180 度
    Rotate180,
    /// 顺时针 270 度（逆时针 90 度）
    CounterClockwise90,
}

impl Rotation {
    ///返回所有可能的旋转。
    #[must_use]
    pub const fn values() -> [Self; 4] {
        [
            Self::None,
            Self::Clockwise90,
            Self::Rotate180,
            Self::CounterClockwise90,
        ]
    }

    /// 根据给定的随机值（0-3）获取一个随机旋转。
    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        match index % 4 {
            0 => Self::None,
            1 => Self::Clockwise90,
            2 => Self::Rotate180,
            _ => Self::CounterClockwise90,
        }
    }

    /// 根据此旋转变换模板边界内的位置。
    ///
    /// 该位置绕 Y 轴旋转。`size` 参数定义
    /// 模板尺寸，用于计算枢轴点。
    #[must_use]
    pub const fn transform_pos(&self, pos: Vector3<i32>, size: Vector3<i32>) -> Vector3<i32> {
        match self {
            Self::None => pos,
            Self::Clockwise90 => Vector3::new(size.z - 1 - pos.z, pos.y, pos.x),
            Self::Rotate180 => Vector3::new(size.x - 1 - pos.x, pos.y, size.z - 1 - pos.z),
            Self::CounterClockwise90 => Vector3::new(pos.z, pos.y, size.x - 1 - pos.x),
        }
    }

    /// 绕原点旋转一个 X/Z 偏移。
    ///
    /// 与会在模板边界内旋转的 `transform_pos` 不同，
    /// 此函数旋转一个简单偏移量（例如子模板定位）。
    #[must_use]
    pub const fn rotate_offset(self, x: i32, z: i32) -> (i32, i32) {
        match self {
            Self::None => (x, z),
            Self::Clockwise90 => (-z, x),
            Self::Rotate180 => (-x, -z),
            Self::CounterClockwise90 => (z, -x),
        }
    }

    /// 根据此旋转变换模板的尺寸维度。
    ///
    /// 对于 90 度和 270 度旋转，X 和 Z 尺寸会互换。
    #[must_use]
    pub const fn transform_size(&self, size: Vector3<i32>) -> Vector3<i32> {
        match self {
            Self::None | Self::Rotate180 => size,
            Self::Clockwise90 | Self::CounterClockwise90 => Vector3::new(size.z, size.y, size.x),
        }
    }

    /// 旋转一个水平朝向。
    ///
    /// 接收朝向字符串（north/south/east/west）并返回旋转后的朝向。
    #[must_use]
    pub fn rotate_facing(&self, facing: &str) -> &'static str {
        match self {
            Self::None => match facing {
                "north" => "north",
                "south" => "south",
                "east" => "east",
                "west" => "west",
                _ => leak_str(facing),
            },
            Self::Clockwise90 => match facing {
                "north" => "east",
                "east" => "south",
                "south" => "west",
                "west" => "north",
                _ => leak_str(facing),
            },
            Self::Rotate180 => match facing {
                "north" => "south",
                "south" => "north",
                "east" => "west",
                "west" => "east",
                _ => leak_str(facing),
            },
            Self::CounterClockwise90 => match facing {
                "north" => "west",
                "west" => "south",
                "south" => "east",
                "east" => "north",
                _ => leak_str(facing),
            },
        }
    }

    /// 旋转一个水平轴。
    ///
    /// 接收轴字符串（x/z）并返回旋转后的轴。
    #[must_use]
    pub fn rotate_axis(&self, axis: &str) -> &'static str {
        match self {
            Self::None | Self::Rotate180 => match axis {
                "x" => "x",
                "z" => "z",
                _ => leak_str(axis),
            },
            Self::Clockwise90 | Self::CounterClockwise90 => match axis {
                "x" => "z",
                "z" => "x",
                _ => leak_str(axis),
            },
        }
    }

    /// 旋转一个方块旋转值（0-15，用于告示牌和旗帜）。
    #[must_use]
    pub const fn rotate_block_rotation(&self, rotation: i32) -> i32 {
        match self {
            Self::None => rotation,
            Self::Clockwise90 => (rotation + 4) % 16,
            Self::Rotate180 => (rotation + 8) % 16,
            Self::CounterClockwise90 => (rotation + 12) % 16,
        }
    }

    /// 将此旋转与另一个旋转组合。
    #[must_use]
    pub const fn then(&self, other: Self) -> Self {
        match self {
            Self::None => other,
            Self::Clockwise90 => match other {
                Self::None => Self::Clockwise90,
                Self::Clockwise90 => Self::Rotate180,
                Self::Rotate180 => Self::CounterClockwise90,
                Self::CounterClockwise90 => Self::None,
            },
            Self::Rotate180 => match other {
                Self::None => Self::Rotate180,
                Self::Clockwise90 => Self::CounterClockwise90,
                Self::Rotate180 => Self::None,
                Self::CounterClockwise90 => Self::Clockwise90,
            },
            Self::CounterClockwise90 => match other {
                Self::None => Self::CounterClockwise90,
                Self::Clockwise90 => Self::None,
                Self::Rotate180 => Self::Clockwise90,
                Self::CounterClockwise90 => Self::Rotate180,
            },
        }
    }

    /// 返回此旋转的逆旋转。
    #[must_use]
    pub const fn inverse(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Clockwise90 => Self::CounterClockwise90,
            Self::Rotate180 => Self::Rotate180,
            Self::CounterClockwise90 => Self::Clockwise90,
        }
    }

    /// 将旋转转换为用于创建边界框的主轴。
    #[must_use]
    pub const fn to_axis(self) -> papokin_util::math::vector3::Axis {
        match self {
            Self::None | Self::Rotate180 => papokin_util::math::vector3::Axis::Z,
            Self::Clockwise90 | Self::CounterClockwise90 => papokin_util::math::vector3::Axis::X,
        }
    }

    /// 根据此旋转绕 Y 轴旋转一个三维方块方向。
    #[must_use]
    pub const fn rotate(&self, direction: BlockDirection) -> BlockDirection {
        if matches!(direction, BlockDirection::Down | BlockDirection::Up) {
            return direction;
        }
        match self {
            Self::None => direction,
            Self::Clockwise90 => direction.rotate_clockwise(),
            Self::Rotate180 => direction.opposite(),
            Self::CounterClockwise90 => direction.rotate_counter_clockwise(),
        }
    }

    /// 按此旋转旋转一个水平朝向。
    #[must_use]
    pub const fn rotate_horizontal(&self, facing: HorizontalFacing) -> HorizontalFacing {
        match self {
            Self::None => facing,
            Self::Clockwise90 => facing.rotate_clockwise(),
            Self::Rotate180 => facing.opposite(),
            Self::CounterClockwise90 => facing.rotate_counter_clockwise(),
        }
    }
}

/// 结构模板的镜像变换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mirror {
    /// 不镜像
    #[default]
    None,
    /// 沿 Z 轴镜像（反转 Z：北 <-> 南）
    LeftRight,
    /// 沿 X 轴镜像（反转 X：东 <-> 西）
    FrontBack,
}

impl Mirror {
    ///返回所有可能的镜像。
    #[must_use]
    pub const fn values() -> [Self; 3] {
        [Self::None, Self::LeftRight, Self::FrontBack]
    }

    /// 根据此镜像变换模板边界内的位置。
    #[must_use]
    pub const fn transform_pos(&self, pos: Vector3<i32>, size: Vector3<i32>) -> Vector3<i32> {
        match self {
            Self::None => pos,
            Self::LeftRight => Vector3::new(pos.x, pos.y, size.z - 1 - pos.z),
            Self::FrontBack => Vector3::new(size.x - 1 - pos.x, pos.y, pos.z),
        }
    }

    /// 镜像一个水平朝向。
    #[must_use]
    pub fn mirror_facing(&self, facing: &str) -> &'static str {
        match self {
            Self::None => match facing {
                "north" => "north",
                "south" => "south",
                "east" => "east",
                "west" => "west",
                _ => leak_str(facing),
            },
            Self::LeftRight => match facing {
                "north" => "south",
                "south" => "north",
                "east" => "east",
                "west" => "west",
                _ => leak_str(facing),
            },
            Self::FrontBack => match facing {
                "east" => "west",
                "west" => "east",
                "north" => "north",
                "south" => "south",
                _ => leak_str(facing),
            },
        }
    }

    /// 镜像一个方块旋转值（0-15，用于告示牌和旗帜）。
    #[must_use]
    pub const fn mirror_block_rotation(&self, rotation: i32) -> i32 {
        match self {
            Self::None => rotation,
            Self::LeftRight => (8 - rotation + 16) % 16,
            Self::FrontBack => (16 - rotation) % 16,
        }
    }

    /// 返回从基础旋转得到此镜像所需的旋转。
    #[must_use]
    pub const fn get_rotation(&self, rotation: Rotation) -> Rotation {
        match self {
            Self::None => rotation,
            Self::LeftRight => match rotation {
                Rotation::None => Rotation::Rotate180,
                Rotation::Clockwise90 => Rotation::Clockwise90,
                Rotation::Rotate180 => Rotation::None,
                Rotation::CounterClockwise90 => Rotation::CounterClockwise90,
            },
            Self::FrontBack => match rotation {
                Rotation::None => Rotation::None,
                Rotation::Clockwise90 => Rotation::CounterClockwise90,
                Rotation::Rotate180 => Rotation::Rotate180,
                Rotation::CounterClockwise90 => Rotation::Clockwise90,
            },
        }
    }

    /// 根据此镜像平面镜像三维方块朝向。
    #[must_use]
    pub const fn mirror(&self, direction: BlockDirection) -> BlockDirection {
        match self {
            Self::None => direction,
            Self::LeftRight => match direction {
                BlockDirection::North => BlockDirection::South,
                BlockDirection::South => BlockDirection::North,
                _ => direction,
            },
            Self::FrontBack => match direction {
                BlockDirection::East => BlockDirection::West,
                BlockDirection::West => BlockDirection::East,
                _ => direction,
            },
        }
    }

    /// 根据此镜像平面镜像水平朝向。
    #[must_use]
    pub const fn mirror_horizontal(&self, facing: HorizontalFacing) -> HorizontalFacing {
        match self {
            Self::None => facing,
            Self::LeftRight => match facing {
                HorizontalFacing::North => HorizontalFacing::South,
                HorizontalFacing::South => HorizontalFacing::North,
                _ => facing,
            },
            Self::FrontBack => match facing {
                HorizontalFacing::East => HorizontalFacing::West,
                HorizontalFacing::West => HorizontalFacing::East,
                _ => facing,
            },
        }
    }

    /// 返回用此镜像方式镜像一个方向所需的旋转。
    #[must_use]
    pub const fn get_rotation_for_direction(&self, direction: BlockDirection) -> Rotation {
        match self {
            Self::LeftRight
                if matches!(direction, BlockDirection::North | BlockDirection::South) =>
            {
                Rotation::Rotate180
            }
            Self::FrontBack if matches!(direction, BlockDirection::East | BlockDirection::West) => {
                Rotation::Rotate180
            }
            _ => Rotation::None,
        }
    }

    /// 返回用此镜像方式镜像一个水平朝向所需的旋转。
    #[must_use]
    pub const fn get_rotation_for_horizontal(&self, facing: HorizontalFacing) -> Rotation {
        match self {
            Self::LeftRight
                if matches!(facing, HorizontalFacing::North | HorizontalFacing::South) =>
            {
                Rotation::Rotate180
            }
            Self::FrontBack
                if matches!(facing, HorizontalFacing::East | HorizontalFacing::West) =>
            {
                Rotation::Rotate180
            }
            _ => Rotation::None,
        }
    }
}

/// 基于旋转和镜像变换铁轨形状，与原版 Minecraft 的 BaseRailBlock 一致。
#[must_use]
pub fn transform_rail_shape(shape: &str, rotation: Rotation, mirror: Mirror) -> String {
    let mut shape = shape;

    match mirror {
        Mirror::LeftRight => {
            shape = match shape {
                "ascending_north" => "ascending_south",
                "ascending_south" => "ascending_north",
                "south_east" => "north_east",
                "south_west" => "north_west",
                "north_west" => "south_west",
                "north_east" => "south_east",
                _ => shape,
            };
        }
        Mirror::FrontBack => {
            shape = match shape {
                "ascending_east" => "ascending_west",
                "ascending_west" => "ascending_east",
                "south_east" => "south_west",
                "south_west" => "south_east",
                "north_west" => "north_east",
                "north_east" => "north_west",
                _ => shape,
            };
        }
        Mirror::None => {}
    }

    match rotation {
        Rotation::Rotate180 => {
            shape = match shape {
                "ascending_east" => "ascending_west",
                "ascending_west" => "ascending_east",
                "ascending_north" => "ascending_south",
                "ascending_south" => "ascending_north",
                "south_east" => "north_west",
                "south_west" => "north_east",
                "north_west" => "south_east",
                "north_east" => "south_west",
                _ => shape,
            };
        }
        Rotation::CounterClockwise90 => {
            shape = match shape {
                "ascending_east" => "ascending_north",
                "ascending_west" => "ascending_south",
                "ascending_north" => "ascending_west",
                "ascending_south" => "ascending_east",
                "north_south" => "east_west",
                "east_west" => "north_south",
                "south_east" => "north_east",
                "south_west" => "south_east",
                "north_west" => "south_west",
                "north_east" => "north_west",
                _ => shape,
            };
        }
        Rotation::Clockwise90 => {
            shape = match shape {
                "ascending_east" => "ascending_south",
                "ascending_west" => "ascending_north",
                "ascending_north" => "ascending_east",
                "ascending_south" => "ascending_west",
                "north_south" => "east_west",
                "east_west" => "north_south",
                "south_east" => "south_west",
                "south_west" => "north_west",
                "north_west" => "north_east",
                "north_east" => "south_east",
                _ => shape,
            };
        }
        Rotation::None => {}
    }

    shape.to_string()
}

/// 基于旋转和镜像变换方块状态属性，与原版 Minecraft 完全一致。
///
/// 原版先执行镜像，再执行旋转。
#[must_use]
pub fn transform_block_properties<K: AsRef<str>, V: AsRef<str>>(
    block_name: &str,
    properties: &[(K, V)],
    rotation: Rotation,
    mirror: Mirror,
) -> Vec<(String, String)> {
    if rotation == Rotation::None && mirror == Mirror::None {
        return properties
            .iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
            .collect();
    }

    let is_stairs = block_name.ends_with("_stairs");
    let is_door = block_name.ends_with("_door") && !block_name.ends_with("trapdoor");
    let is_rail = block_name == "minecraft:rail"
        || block_name == "rail"
        || block_name == "minecraft:powered_rail"
        || block_name == "powered_rail"
        || block_name == "minecraft:detector_rail"
        || block_name == "detector_rail"
        || block_name == "minecraft:activator_rail"
        || block_name == "activator_rail";

    if is_stairs {
        let mut facing_opt = None;
        let mut shape_opt = None;
        for (k, v) in properties {
            if k.as_ref() == "facing" {
                facing_opt = Some(v.as_ref());
            } else if k.as_ref() == "shape" {
                shape_opt = Some(v.as_ref());
            }
        }

        let (new_facing, new_shape) = if let Some(facing) = facing_opt {
            let shape = shape_opt.unwrap_or("straight");
            let mut f = facing;
            let mut s = shape;

            // 步骤 1：StairBlock.mirror
            match mirror {
                Mirror::LeftRight => {
                    if f == "north" || f == "south" {
                        f = match f {
                            "north" => "south",
                            "south" => "north",
                            _ => f,
                        };
                        s = match s {
                            "straight" => "straight",
                            "outer_left" => "outer_right",
                            "inner_right" => "inner_left",
                            "inner_left" => "inner_right",
                            "outer_right" => "outer_left",
                            _ => s,
                        };
                    }
                }
                Mirror::FrontBack => {
                    if f == "east" || f == "west" {
                        f = match f {
                            "east" => "west",
                            "west" => "east",
                            _ => f,
                        };
                        s = match s {
                            "straight" => "straight",
                            "outer_left" => "outer_right",
                            "inner_right" => "inner_right",
                            "inner_left" => "inner_left",
                            "outer_right" => "outer_left",
                            _ => s,
                        };
                    }
                }
                Mirror::None => {}
            }

            // 步骤 2：StairBlock.rotate（仅旋转朝向）
            f = rotation.rotate_facing(f);

            (Some(f.to_string()), Some(s.to_string()))
        } else {
            (None, None)
        };

        return properties
            .iter()
            .map(|(k, v)| {
                let k_str = k.as_ref();
                if k_str == "facing" {
                    if let Some(ref nf) = new_facing {
                        return (k_str.to_string(), nf.clone());
                    }
                } else if k_str == "shape" {
                    if let Some(ref ns) = new_shape {
                        return (k_str.to_string(), ns.clone());
                    }
                }
                (k_str.to_string(), v.as_ref().to_string())
            })
            .collect();
    }

    if is_door {
        let mut facing_opt = None;
        let mut hinge_opt = None;
        for (k, v) in properties {
            if k.as_ref() == "facing" {
                facing_opt = Some(v.as_ref());
            } else if k.as_ref() == "hinge" {
                hinge_opt = Some(v.as_ref());
            }
        }

        let (new_facing, new_hinge) = if let Some(facing) = facing_opt {
            let hinge = hinge_opt.unwrap_or("left");
            let mut f = facing;
            let mut h = hinge;

            // 步骤 1：DoorBlock.mirror
            if mirror != Mirror::None {
                f = mirror.mirror_facing(f);
                h = match h {
                    "left" => "right",
                    "right" => "left",
                    _ => h,
                };
            }

            // 步骤 2：DoorBlock.rotate
            f = rotation.rotate_facing(f);

            (Some(f.to_string()), Some(h.to_string()))
        } else {
            (None, None)
        };

        return properties
            .iter()
            .map(|(k, v)| {
                let k_str = k.as_ref();
                if k_str == "facing" {
                    if let Some(ref nf) = new_facing {
                        return (k_str.to_string(), nf.clone());
                    }
                } else if k_str == "hinge" {
                    if let Some(ref nh) = new_hinge {
                        return (k_str.to_string(), nh.clone());
                    }
                }
                (k_str.to_string(), v.as_ref().to_string())
            })
            .collect();
    }

    if is_rail {
        return properties
            .iter()
            .map(|(k, v)| {
                let k_str = k.as_ref();
                if k_str == "shape" {
                    let s = transform_rail_shape(v.as_ref(), rotation, mirror);
                    (k_str.to_string(), s)
                } else {
                    (k_str.to_string(), v.as_ref().to_string())
                }
            })
            .collect();
    }

    // 通用属性变换
    properties
        .iter()
        .map(|(k, v)| {
            let key = k.as_ref();
            let val = v.as_ref();

            let transformed_key = match key {
                "north" | "south" | "east" | "west" => rotation
                    .rotate_facing(mirror.mirror_facing(key))
                    .to_string(),
                _ => key.to_string(),
            };

            let transformed_val = match key {
                "facing" => {
                    let mirrored = mirror.mirror_facing(val);
                    rotation.rotate_facing(mirrored).to_string()
                }
                "orientation" => {
                    let mut parts = val.split('_');
                    if let (Some(front), Some(top)) = (parts.next(), parts.next()) {
                        let mirrored_front = mirror.mirror_facing(front);
                        let rotated_front = rotation.rotate_facing(mirrored_front);
                        let mirrored_top = mirror.mirror_facing(top);
                        let rotated_top = rotation.rotate_facing(mirrored_top);
                        format!("{rotated_front}_{rotated_top}")
                    } else {
                        val.to_string()
                    }
                }
                "axis" => rotation.rotate_axis(val).to_string(),
                "rotation" => val.parse::<i32>().map_or_else(
                    |_| val.to_string(),
                    |rot_value| {
                        let mirrored = mirror.mirror_block_rotation(rot_value);
                        let rotated = rotation.rotate_block_rotation(mirrored);
                        rotated.to_string()
                    },
                ),
                _ => val.to_string(),
            };

            (transformed_key, transformed_val)
        })
        .collect()
}

/// 泄漏一个字符串以获得 'static str。
/// 这用于静态字符串未涵盖的非标准属性值。
pub fn leak_str(s: &str) -> &'static str {
    s.to_string().leak()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_mirror_left_right() {
        // LeftRight 反转 Z（北 <-> 南）
        let props = [("facing", "north"), ("shape", "inner_left")];
        let transformed = transform_block_properties(
            "minecraft:oak_stairs",
            &props,
            Rotation::None,
            Mirror::LeftRight,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        assert_eq!(facing, Some("south"));
        assert_eq!(shape, Some("inner_right"));

        // LeftRight 不改变朝向东/西的楼梯形状或朝向
        let props = [("facing", "east"), ("shape", "inner_left")];
        let transformed = transform_block_properties(
            "minecraft:oak_stairs",
            &props,
            Rotation::None,
            Mirror::LeftRight,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        assert_eq!(facing, Some("east"));
        assert_eq!(shape, Some("inner_left"));
    }

    #[test]
    fn stairs_mirror_front_back() {
        // FrontBack 反转 X（东 <-> 西）
        let props = [("facing", "east"), ("shape", "outer_left")];
        let transformed = transform_block_properties(
            "minecraft:oak_stairs",
            &props,
            Rotation::None,
            Mirror::FrontBack,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        assert_eq!(facing, Some("west"));
        assert_eq!(shape, Some("outer_right"));

        // FrontBack 不改变朝向北/南的楼梯
        let props = [("facing", "north"), ("shape", "outer_left")];
        let transformed = transform_block_properties(
            "minecraft:oak_stairs",
            &props,
            Rotation::None,
            Mirror::FrontBack,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        assert_eq!(facing, Some("north"));
        assert_eq!(shape, Some("outer_left"));
    }

    #[test]
    fn stairs_mirror_and_rotate() {
        // 先 LeftRight 镜像再顺时针旋转 90 度
        let props = [("facing", "north"), ("shape", "inner_left")];
        let transformed = transform_block_properties(
            "minecraft:oak_stairs",
            &props,
            Rotation::Clockwise90,
            Mirror::LeftRight,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        // 北 -> 南（经 LeftRight 镜像）-> 西（经 Clockwise90 旋转）
        assert_eq!(facing, Some("west"));
        // inner_left -> inner_right（通过 LeftRight 镜像，旋转不改变其结果）
        assert_eq!(shape, Some("inner_right"));
    }

    #[test]
    fn door_mirror_and_rotate() {
        let props = [("facing", "east"), ("hinge", "left")];
        let transformed = transform_block_properties(
            "minecraft:oak_door",
            &props,
            Rotation::None,
            Mirror::FrontBack,
        );
        let facing = transformed
            .iter()
            .find(|(k, _)| k == "facing")
            .map(|(_, v)| v.as_str());
        let hinge = transformed
            .iter()
            .find(|(k, _)| k == "hinge")
            .map(|(_, v)| v.as_str());
        assert_eq!(facing, Some("west"));
        assert_eq!(hinge, Some("right"));
    }

    #[test]
    fn rail_mirror_and_rotate() {
        let props = [("shape", "north_east")];
        let transformed = transform_block_properties(
            "minecraft:rail",
            &props,
            Rotation::Clockwise90,
            Mirror::LeftRight,
        );
        let shape = transformed
            .iter()
            .find(|(k, _)| k == "shape")
            .map(|(_, v)| v.as_str());
        assert_eq!(shape, Some("south_west"));
    }
}
