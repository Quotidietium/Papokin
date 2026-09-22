//! 从 Java 版与未命名网络 NBT 反序列化。

use std::borrow::Cow;
use std::io::{Cursor, Seek, SeekFrom};

use crate::{Error, io};
use io::Read;

/// NBT 反序列化操作返回的结果类型。
pub type Result<T> = std::result::Result<T, Error>;

/// NBT 读取辅助函数所使用的字节源。
///
/// 在……情况下，实现可以返回借用的字符串和字节数组
/// 在底层存储允许的情况下。
pub trait NbtDataSource<'a> {
    /// 读取一个无符号字节。
    fn read_u8(&mut self) -> Result<u8>;
    /// 用来自源的字节填充 `buf`。
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()>;
    /// 将当前位置移动 `offset` 字节。
    fn seek_relative(&mut self, offset: i64) -> Result<()>;
    /// 读取并解码包含 `len` 个字节的字符串负载。
    fn read_string(&mut self, len: usize) -> Result<Cow<'a, str>>;
    /// 读取包含 `len` 个元素的字节数组负载。
    fn read_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>>;
}

/// 将 [`Read`] 和 [`Seek`] 流适配为 [`NbtDataSource`]。
pub struct NbtStreamReader<R>(
    /// 包装后的输入流。
    pub R,
);

impl<'a, R: Read + Seek> NbtDataSource<'a> for NbtStreamReader<R> {
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.0.read_exact(&mut buf).map_err(Error::Incomplete)?;
        Ok(buf[0])
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        self.0.read_exact(buf).map_err(Error::Incomplete)
    }

    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        self.0
            .seek(SeekFrom::Current(offset))
            .map_err(Error::Incomplete)?;
        Ok(())
    }

    fn read_string(&mut self, len: usize) -> Result<Cow<'a, str>> {
        let mut buf = vec![0u8; len];
        self.0.read_exact(&mut buf).map_err(Error::Incomplete)?;
        let string = cesu8::from_java_cesu8(&buf).map_err(|_| Error::Cesu8DecodingError)?;
        Ok(Cow::Owned(string.into_owned()))
    }

    fn read_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>> {
        let mut buf = vec![0u8; len];
        self.0.read_exact(&mut buf).map_err(Error::Incomplete)?;
        let i8_buf: Vec<i8> = buf.into_iter().map(|b| b as i8).collect();
        Ok(Cow::Owned(i8_buf))
    }
}

impl<'a> NbtDataSource<'a> for Cursor<&'a [u8]> {
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf).map_err(Error::Incomplete)?;
        Ok(buf[0])
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        self.read_exact(buf).map_err(Error::Incomplete)
    }

    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        self.seek(SeekFrom::Current(offset))
            .map_err(Error::Incomplete)?;
        Ok(())
    }

    fn read_string(&mut self, len: usize) -> Result<Cow<'a, str>> {
        let pos = self.position() as usize;
        let data_len = self.get_ref().len();
        if pos + len > data_len {
            return Err(Error::Incomplete(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            )));
        }
        self.set_position((pos + len) as u64);
        let data = self.get_ref();
        let slice = &data[pos..pos + len];
        if let Ok(s) = std::str::from_utf8(slice) {
            Ok(Cow::Borrowed(s))
        } else {
            let string = cesu8::from_java_cesu8(slice).map_err(|_| Error::Cesu8DecodingError)?;
            Ok(string)
        }
    }

    fn read_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>> {
        let pos = self.position() as usize;
        let data_len = self.get_ref().len();
        if pos + len > data_len {
            return Err(Error::Incomplete(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            )));
        }
        self.set_position((pos + len) as u64);
        let data = self.get_ref();
        let slice = &data[pos..pos + len];
        // SAFETY: `slice` 是长度为 `len` 的有效字节切片。`u8` 与 `i8` 具有相同的大小、对齐（1 字节）和有效的值表示。
        let i8_slice = unsafe { std::slice::from_raw_parts(slice.as_ptr().cast::<i8>(), len) };
        Ok(Cow::Borrowed(i8_slice))
    }
}

impl<'a, S: NbtDataSource<'a> + ?Sized> NbtDataSource<'a> for &mut S {
    fn read_u8(&mut self) -> Result<u8> {
        (**self).read_u8()
    }
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_bytes(buf)
    }
    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        (**self).seek_relative(offset)
    }
    fn read_string(&mut self, len: usize) -> Result<Cow<'a, str>> {
        (**self).read_string(len)
    }
    fn read_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>> {
        (**self).read_byte_array(len)
    }
}

impl<'a> NbtDataSource<'a> for Cursor<Vec<u8>> {
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf).map_err(Error::Incomplete)?;
        Ok(buf[0])
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        self.read_exact(buf).map_err(Error::Incomplete)
    }

    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        self.seek(SeekFrom::Current(offset))
            .map_err(Error::Incomplete)?;
        Ok(())
    }

    fn read_string(&mut self, len: usize) -> Result<Cow<'a, str>> {
        let pos = self.position() as usize;
        let data_len = self.get_ref().len();
        if pos + len > data_len {
            return Err(Error::Incomplete(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            )));
        }
        self.set_position((pos + len) as u64);
        let data = self.get_ref();
        let slice = &data[pos..pos + len];
        let string = cesu8::from_java_cesu8(slice).map_err(|_| Error::Cesu8DecodingError)?;
        Ok(Cow::Owned(string.into_owned()))
    }

    fn read_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>> {
        let pos = self.position() as usize;
        let data_len = self.get_ref().len();
        if pos + len > data_len {
            return Err(Error::Incomplete(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected EOF",
            )));
        }
        self.set_position((pos + len) as u64);
        let data = self.get_ref();
        let slice = &data[pos..pos + len];
        // SAFETY: `slice` 是长度为 `len` 的有效字节切片。`u8` 与 `i8` 具有相同的大小、对齐（1 字节）和有效的值表示。
        let i8_slice = unsafe { std::slice::from_raw_parts(slice.as_ptr().cast::<i8>(), len) };
        Ok(Cow::Owned(i8_slice.to_vec()))
    }
}

/// NBT 解析器使用的特定格式原始类型读取器。
pub trait NbtReadHelper<'a> {
    /// 底层字节源。
    type Reader: NbtDataSource<'a>;

    /// 返回底层的字节源。
    fn reader(&mut self) -> &mut Self::Reader;

    /// 前进 `count` 个字节。
    fn skip_bytes(&mut self, count: i64) -> Result<()> {
        self.reader().seek_relative(count)
    }
    /// 跳过一个无符号字节。
    fn skip_u8(&mut self) -> Result<()> {
        self.skip_bytes(1)
    }
    /// 跳过一个有符号字节。
    fn skip_i8(&mut self) -> Result<()> {
        self.skip_bytes(1)
    }
    /// 跳过一个 16 位有符号整数。
    fn skip_i16(&mut self) -> Result<()> {
        self.skip_bytes(2)
    }
    /// 跳过一个 32 位有符号整数。
    fn skip_i32(&mut self) -> Result<()> {
        self.skip_bytes(4)
    }
    /// 跳过一个 64 位有符号整数。
    fn skip_i64(&mut self) -> Result<()> {
        self.skip_bytes(8)
    }
    /// 跳过一个 32 位浮点数。
    fn skip_f32(&mut self) -> Result<()> {
        self.skip_bytes(4)
    }
    /// 跳过一个 64 位浮点数。
    fn skip_f64(&mut self) -> Result<()> {
        self.skip_bytes(8)
    }
    /// 跳过一个带长度前缀的字符串。
    fn skip_string(&mut self) -> Result<()>;

    /// 读取一个无符号字节。
    fn get_u8(&mut self) -> Result<u8>;
    /// 读取一个有符号字节。
    fn get_i8(&mut self) -> Result<i8>;
    /// 读取一个 16 位有符号整数。
    fn get_i16(&mut self) -> Result<i16>;
    /// 读取一个 32 位有符号整数。
    fn get_i32(&mut self) -> Result<i32>;
    /// 读取一个 64 位有符号整数。
    fn get_i64(&mut self) -> Result<i64>;
    /// 读取一个 32 位有符号整数数组。
    fn get_i32_array(&mut self, len: usize) -> Result<Vec<i32>> {
        let mut values = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            values.push(self.get_i32()?);
        }
        Ok(values)
    }
    /// 读取一个 64 位有符号整数数组。
    fn get_i64_array(&mut self, len: usize) -> Result<Vec<i64>> {
        let mut values = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            values.push(self.get_i64()?);
        }
        Ok(values)
    }
    /// 读取一个 32 位浮点数。
    fn get_f32(&mut self) -> Result<f32>;
    /// 读取一个 64 位浮点数。
    fn get_f64(&mut self) -> Result<f64>;
    /// 读取一个带长度前缀的字符串。
    fn get_string(&mut self) -> Result<Cow<'a, str>>;
    /// 按给定的元素数量读取一个字节数组。
    fn get_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>>;
}

/// 以大端序数字编码读取 Java 版 NBT 基本类型。
pub struct NbtReadHelperJava<D> {
    reader: D,
}

impl<D> NbtReadHelperJava<D> {
    /// 基于 `r` 创建 Java 版读取器。
    pub const fn new(r: D) -> Self {
        Self { reader: r }
    }
}

impl<'a, D: NbtDataSource<'a>> NbtReadHelperJava<D> {
    fn get_string_len(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.reader.read_bytes(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
}

impl<'a, D: NbtDataSource<'a>> NbtReadHelper<'a> for NbtReadHelperJava<D> {
    type Reader = D;

    fn reader(&mut self) -> &mut D {
        &mut self.reader
    }

    fn skip_string(&mut self) -> Result<()> {
        let len = self.get_string_len()? as i64;
        self.skip_bytes(len)
    }

    fn get_u8(&mut self) -> Result<u8> {
        self.reader.read_u8()
    }
    fn get_i8(&mut self) -> Result<i8> {
        Ok(self.reader.read_u8()? as i8)
    }
    fn get_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.reader.read_bytes(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }
    fn get_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.reader.read_bytes(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }
    fn get_i64(&mut self) -> Result<i64> {
        let mut buf = [0u8; 8];
        self.reader.read_bytes(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }
    fn get_i32_array(&mut self, len: usize) -> Result<Vec<i32>> {
        let byte_len = len
            .checked_mul(std::mem::size_of::<i32>())
            .ok_or(Error::LargeLength(len))?;
        let mut values = Vec::<i32>::with_capacity(len);
        // SAFETY: 该向量具有容纳 `byte_len` 字节的容量，且 `u8` 接受
        // 每个位模式。读取成功前其长度保持为零。
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(values.as_mut_ptr().cast::<u8>(), byte_len) };
        self.reader.read_bytes(bytes)?;
        // SAFETY: 所有 `len` 元素的每个字节均由 `read_bytes` 初始化，
        // 且每个位模式都是合法的 `i32`。
        unsafe { values.set_len(len) };
        for value in &mut values {
            *value = value.to_be();
        }
        Ok(values)
    }
    fn get_i64_array(&mut self, len: usize) -> Result<Vec<i64>> {
        let byte_len = len
            .checked_mul(std::mem::size_of::<i64>())
            .ok_or(Error::LargeLength(len))?;
        let mut values = Vec::<i64>::with_capacity(len);
        // SAFETY: 该向量具有容纳 `byte_len` 字节的容量，且 `u8` 接受
        // 每个位模式。读取成功前其长度保持为零。
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(values.as_mut_ptr().cast::<u8>(), byte_len) };
        self.reader.read_bytes(bytes)?;
        // SAFETY: 所有 `len` 元素的每个字节均由 `read_bytes` 初始化，
        // 且每个位模式都是合法的 `i64`。
        unsafe { values.set_len(len) };
        for value in &mut values {
            *value = value.to_be();
        }
        Ok(values)
    }
    fn get_f32(&mut self) -> Result<f32> {
        let mut buf = [0u8; 4];
        self.reader.read_bytes(&mut buf)?;
        Ok(f32::from_be_bytes(buf))
    }
    fn get_f64(&mut self) -> Result<f64> {
        let mut buf = [0u8; 8];
        self.reader.read_bytes(&mut buf)?;
        Ok(f64::from_be_bytes(buf))
    }

    fn get_string(&mut self) -> Result<Cow<'a, str>> {
        let len = self.get_string_len()? as usize;
        self.reader.read_string(len)
    }

    fn get_byte_array(&mut self, len: usize) -> Result<Cow<'a, [i8]>> {
        self.reader.read_byte_array(len)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{NbtReadHelper, NbtReadHelperJava};

    #[test]
    fn java_numeric_arrays_decode_big_endian_values() {
        let ints = [i32::MIN, -1, 0, 1, i32::MAX];
        let int_bytes: Vec<u8> = ints.iter().flat_map(|value| value.to_be_bytes()).collect();
        let mut reader = NbtReadHelperJava::new(Cursor::new(int_bytes.as_slice()));
        assert_eq!(reader.get_i32_array(ints.len()).unwrap(), ints);

        let longs = [i64::MIN, -1, 0, 1, i64::MAX];
        let long_bytes: Vec<u8> = longs.iter().flat_map(|value| value.to_be_bytes()).collect();
        let mut reader = NbtReadHelperJava::new(Cursor::new(long_bytes.as_slice()));
        assert_eq!(reader.get_i64_array(longs.len()).unwrap(), longs);
    }
}
