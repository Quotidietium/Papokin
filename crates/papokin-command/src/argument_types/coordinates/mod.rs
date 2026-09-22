use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::{CommandErrorType, READER_EXPECTED_DOUBLE, READER_EXPECTED_INT};
use crate::source::CommandSource;
use crate::string_reader::StringReader;
use papokin_data::translation;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::{Axis, Vector3};

pub mod angle;
pub mod block_pos;
pub mod column_pos;
pub mod rotation;
pub mod swizzle;
pub mod vec2;
pub mod vec3;

pub const MIXED_TYPE_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_POS_MIXED);

/// 表示单个世界坐标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorldCoordinate {
    Absolute(f64),
    Relative(f64),
}

impl WorldCoordinate {
    /// 创建新的 `WorldCoordinate`。
    #[must_use]
    pub const fn new(is_relative: bool, value: f64) -> Self {
        if is_relative {
            Self::Relative(value)
        } else {
            Self::Absolute(value)
        }
    }

    /// 返回此坐标是否为相对坐标。
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        matches!(self, Self::Relative(_))
    }

    /// 返回此 [`WorldCoordinate`] 所表示的物理坐标值，在给定
    /// 一个绝对坐标原点。
    #[must_use]
    pub const fn resolve(&self, origin: f64) -> f64 {
        match self {
            Self::Absolute(absolute) => *absolute,
            Self::Relative(relative) => origin + *relative,
        }
    }

    /// 检查 `StringReader` 是否即将描述相对坐标。
    ///
    /// # Arguments
    /// * `reader` - 要检查的 `StringReader`。
    ///
    /// # Returns
    /// - 若能找到 `~`（波浪号）则为 `true`。该方法也会将其跳过。
    /// - 若找不到 `~` 则返回 `false`。
    pub fn consume_relative_start(reader: &mut StringReader) -> bool {
        if reader.peek() == Some('~') {
            reader.skip();
            true
        } else {
            false
        }
    }

    /// 尝试从单个数字解析一个 [`WorldCoordinate`]。
    ///
    /// # Arguments
    /// * `reader` - 用于解析单个坐标的 `StringReader`。
    /// * `center_integers` - 是否通过加上 `+0.5` 来修正整数坐标
    ///   (如 `Vec3ArgumentType::Default` 所述)。
    ///
    /// # Returns
    /// - 若解析正确，则返回包裹在 `Ok` 中的 `WorldCoordinate`。
    /// - 一个 [`CommandSyntaxError`]，若未能正确解析则描述错误，
    ///   包装在 `Err` 中。
    pub fn parse(
        reader: &mut StringReader,
        center_integers: bool,
    ) -> Result<Self, CommandSyntaxError> {
        if reader.peek() == Some('^') {
            Err(MIXED_TYPE_ERROR_TYPE.create(reader))
        } else if !reader.can_read_char() {
            Err(READER_EXPECTED_DOUBLE.create(reader))
        } else {
            let is_relative = Self::consume_relative_start(reader);
            let i = reader.cursor();
            let mut value = if reader.can_read_char() && reader.peek() != Some(' ') {
                reader.read_double()?
            } else {
                0.0
            };
            let slice = &reader.string()[i..reader.cursor()];
            if is_relative && slice.is_empty() {
                Ok(Self::Relative(0.0))
            } else {
                if !slice.contains('.') && !is_relative && center_integers {
                    value += 0.5;
                }
                Ok(Self::new(is_relative, value))
            }
        }
    }

    /// 尝试从单个数字解析 [`WorldCoordinate`]，期望为整数型的非相对坐标
    /// 或任意相对坐标。
    ///
    /// # Arguments
    /// * `reader` - 用于解析单个坐标的 `StringReader`。
    ///
    /// # Returns
    /// - 若解析正确，则返回包裹在 `Ok` 中的 `WorldCoordinate`。
    /// - 一个 [`CommandSyntaxError`]，若未能正确解析则描述错误，
    ///   包装在 `Err` 中。
    pub fn parse_integer(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        if reader.peek() == Some('^') {
            Err(MIXED_TYPE_ERROR_TYPE.create(reader))
        } else if !reader.can_read_char() {
            Err(READER_EXPECTED_INT.create(reader))
        } else {
            let is_relative = Self::consume_relative_start(reader);
            let value = if reader.can_read_char() && reader.peek() != Some(' ') {
                if is_relative {
                    reader.read_double()?
                } else {
                    reader.read_int()? as f64
                }
            } else {
                0.0
            };
            Ok(Self::new(is_relative, value))
        }
    }
}

/// 表示命令坐标的对象。
///
/// 一组 [`Coordinates`] 可以通过 [`Coordinates::resolve`] 方法进行*解析*。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coordinates {
    /// 普通坐标（每个坐标可以是 *绝对* 或 *相对* 的。）
    World(Vector3<WorldCoordinate>),
    /// 局部坐标（可能因命令来源不同而不同。）
    Local { left: f64, up: f64, forward: f64 },
}

macro_rules! check_for_space_char {
    ($reader:ident, $i:ident, $value:ident) => {
        if $reader.peek() == Some(' ') {
            $reader.skip();
            Ok($value)
        } else {
            $reader.set_cursor($i);
            Err(vec3::INCOMPLETE_ERROR_TYPE.create($reader))
        }
    };
}

impl Coordinates {
    /// 返回这些 [`Coordinates`] 中给定 [`Axis`] 的坐标是否为相对坐标。
    ///
    /// 对于*局部坐标*也会返回 `true`。
    #[must_use]
    pub const fn is_relative(&self, axis: Axis) -> bool {
        match self {
            Self::World(vector) => vector.get_axis(axis).is_relative(),
            Self::Local { .. } => true,
        }
    }

    /// 返回这些 [`Coordinates`] 所表示的物理位置。
    #[must_use]
    pub fn resolve(&self, source: &impl CommandSource) -> Vector3<f64> {
        match self {
            Self::World(vector) => {
                let pos = source.position();
                Vector3::new(
                    vector.x.resolve(pos.x),
                    vector.y.resolve(pos.y),
                    vector.z.resolve(pos.z),
                )
            }
            Self::Local { left, up, forward } => {
                let start = source.entity_anchor().position_at_source(source);
                convert_local_coordinates(*left, *up, *forward, source.rotation()).add(&start)
            }
        }
    }

    /// 以 [`Vector2`] 形式返回这些 [`Coordinates`] 所表示的旋转。
    ///
    /// - 返回向量的 *x* 分量是**俯仰角**。
    /// - 返回向量的 *y* 分量是**偏航角**。
    #[must_use]
    pub fn rotation(&self, source: &impl CommandSource) -> Vector2<f32> {
        match self {
            Self::World(coords) => {
                let rotation = source.rotation();
                Vector2::new(
                    coords.x.resolve(rotation.x as f64) as f32,
                    coords.y.resolve(rotation.y as f64) as f32,
                )
            }
            Self::Local { .. } => Vector2::new(0.0, 0.0),
        }
    }

    /// 尝试解析一组世界 [`Coordinates`]，期望每个坐标要么是
    /// 一个整数非相对坐标或任意相对坐标。
    ///
    /// # Arguments
    /// * `reader` - 用于解析坐标的 `StringReader`。
    /// * `center_integers` - 是否通过加上 `+0.5` 来修正整数坐标
    ///   (如 `Vec3ArgumentType::Default` 所述)。
    ///
    /// # Returns
    /// - 若解析正确，则返回包裹在 `Ok` 中的世界 `Coordinates`。
    /// - 一个 [`CommandSyntaxError`]，若未能正确解析则描述错误，
    ///   包装在 `Err` 中。
    pub fn parse_world(
        reader: &mut StringReader,
        center_integers: bool,
    ) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let coordinate_1 = Self::parse_world_single(i, reader, center_integers)?;
        // Y 坐标从不居中。
        let coordinate_2 = Self::parse_world_single(i, reader, false)?;
        let coordinate_3 = WorldCoordinate::parse(reader, center_integers)?;
        Ok(Self::World(Vector3::new(
            coordinate_1,
            coordinate_2,
            coordinate_3,
        )))
    }

    fn parse_world_single(
        i: usize,
        reader: &mut StringReader,
        center_integers: bool,
    ) -> Result<WorldCoordinate, CommandSyntaxError> {
        let coordinate = WorldCoordinate::parse(reader, center_integers)?;
        check_for_space_char!(reader, i, coordinate)
    }

    /// 尝试解析一组世界 [`Coordinates`]。
    ///
    /// # Arguments
    /// * `reader` - 用于解析单个坐标的 `StringReader`。
    ///
    /// # Returns
    /// - 若解析正确，则返回包裹在 `Ok` 中的世界 `Coordinates`。
    /// - 一个 [`CommandSyntaxError`]，若未能正确解析则描述错误，
    ///   包装在 `Err` 中。
    pub fn parse_world_integers(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let coordinate_1 = Self::parse_world_single_integer(i, reader)?;
        let coordinate_2 = Self::parse_world_single_integer(i, reader)?;
        let coordinate_3 = WorldCoordinate::parse_integer(reader)?;
        Ok(Self::World(Vector3::new(
            coordinate_1,
            coordinate_2,
            coordinate_3,
        )))
    }

    fn parse_world_single_integer(
        i: usize,
        reader: &mut StringReader,
    ) -> Result<WorldCoordinate, CommandSyntaxError> {
        let coordinate = WorldCoordinate::parse_integer(reader)?;
        check_for_space_char!(reader, i, coordinate)
    }

    /// 尝试解析一组局部 [`Coordinates`]。
    ///
    /// # Arguments
    /// * `reader` - 用于解析单个坐标的 `StringReader`。
    ///
    /// # Returns
    /// - 若解析正确，则返回包裹在 `Ok` 中的本地 `Coordinates`。
    /// - 一个 [`CommandSyntaxError`]，若未能正确解析则描述错误，
    ///   包装在 `Err` 中。
    pub fn parse_local(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let left = Self::parse_local_single(i, reader)?;
        let up = Self::parse_local_single(i, reader)?;
        let forward = Self::parse_local_number(i, reader)?;
        Ok(Self::Local { left, up, forward })
    }

    fn parse_local_single(i: usize, reader: &mut StringReader) -> Result<f64, CommandSyntaxError> {
        let number = Self::parse_local_number(i, reader)?;
        check_for_space_char!(reader, i, number)
    }

    fn parse_local_number(i: usize, reader: &mut StringReader) -> Result<f64, CommandSyntaxError> {
        if !reader.can_read_char() {
            Err(READER_EXPECTED_DOUBLE.create(reader))
        } else if reader.peek() != Some('^') {
            reader.set_cursor(i);
            Err(MIXED_TYPE_ERROR_TYPE.create(reader))
        } else {
            reader.skip();
            let number = if reader.can_read_char() && reader.peek() != Some(' ') {
                reader.read_double()?
            } else {
                0.0
            };
            Ok(number)
        }
    }
}

/// 将一组局部坐标转换为其物理 [`Vector3`] 形式。
///
/// # Arguments
/// * `left` - 坐标的左向分量。
/// * `up` - 坐标的向上分量。
/// * `forward` - 坐标的前向分量。
/// * `rotation` - 用于计算物理坐标的旋转。
///   两个坐标都必须以 *度* 为单位。
///
/// # Returns
/// 局部坐标所表示的物理位置。
#[must_use]
fn convert_local_coordinates(
    left: f64,
    up: f64,
    forward: f64,
    rotation: Vector2<f32>,
) -> Vector3<f64> {
    let pitch = rotation.x;
    let yaw = rotation.y;

    let y = (yaw + 90.0).to_radians() as f64;
    let y_cos = y.cos();
    let y_sin = y.sin();
    let x = (-pitch).to_radians() as f64;
    let x_cos = x.cos();
    let x_sin = x.sin();
    let x_up = (-pitch + 90.0).to_radians() as f64;
    let x_up_cos = x_up.cos();
    let x_up_sin = x_up.sin();

    let forward_vector = Vector3::new(y_cos * x_cos, x_sin, y_sin * x_cos);
    let up_vector = Vector3::new(y_cos * x_up_cos, x_up_sin, y_sin * x_up_cos);
    let left_vector = forward_vector.cross(&up_vector) * -1.0;

    Vector3::new(
        forward_vector.x * forward + up_vector.x * up + left_vector.x * left,
        forward_vector.y * forward + up_vector.y * up + left_vector.y * left,
        forward_vector.z * forward + up_vector.z * up + left_vector.z * left,
    )
}
