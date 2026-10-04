//! 每连接收发 scratch 内存基准（性能优化轮次 2，见 note/report/perf/）。
//!
//! 工作负载模拟 64 条并发连接的生命周期：
//! 「登录突发（1 MiB 不可压缩大包出入各一）→ 空闲窗口
//! （32 个零负载心跳，模拟 AFK）→ 稳态游玩（2000 轮 64 B
//! 小包 + 每 100 轮一个 128 KiB 类区块中包）」。
//! 空闲窗口是旧实现的驻留放大器：零负载包不触发 reserve，
//! 解码 scratch 会一直抱住大包分配不放。
//!
//! 两种模式：
//! - `new`：当前实现（256 KiB 留存上限 + 防振荡收缩/换新）。
//! - `legacy`：基准内复刻 0.3.14 旧实现（scratch 只增不减、
//!   解码 scratch 绝不换新），同一工作负载。
//!
//! 完整性闸门：大包校验和、小包抽样逐字节比对、两模式
//! 编码输出总字节数必须完全相等（线上字节不变）。
//!
//! 复刻忠实度说明：legacy 解码路径为解析帧头引入一个瞬时
//! 解压 Vec（生产旧代码流式读入 scratch，无此分配），该
//! 差异只让 legacy 吞吐略吃亏、不影响留存账，结论偏保守。

#![allow(clippy::print_stdout)]

use std::{
    error::Error,
    io::{Read as _, Write as _},
    pin::Pin,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Instant,
};

use bytes::{Bytes, BytesMut};
use flate2::{Compress, Compression, FlushCompress, Status, write::ZlibEncoder};
use papokin_protocol::java::{
    packet_decoder::TCPNetworkDecoder, packet_encoder::TCPNetworkEncoder,
};
use serde_json::{Value, json};
use tokio::io::AsyncWrite;

/// 并发连接数（模拟一个中型服务器的在线规模）
const CONNECTIONS: usize = 64;
/// 登录突发大包尺寸（不可压缩，约等于满载区块批/大插件消息）
const BIG_PACKET: usize = 1024 * 1024;
/// 空闲窗口心跳数（零负载，仅包 id）
const HEARTBEATS: usize = 32;
/// 稳态轮次
const ROUNDS: usize = 2000;
/// 稳态小包尺寸（低于压缩阈值，对应移动/心跳类流量）
const SMALL_PACKET: usize = 64;
/// 类区块中包尺寸（高于阈值，走压缩路径）
const MEDIUM_PACKET: usize = 128 * 1024;
/// 每多少轮插入一个中包
const MEDIUM_EVERY: usize = 100;
/// 压缩阈值与等级（对齐生产配置）
const COMPRESSION_THRESHOLD: usize = 256;
const COMPRESSION_LEVEL: u32 = 6;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--child" {
        let rt = tokio::runtime::Runtime::new()?;
        let result = rt.block_on(run_child(&args[2]))?;
        println!("{result}");
        return Ok(());
    }
    run_parent()
}

/// 长度转 i32（帧头字段，基准尺寸远小于 i32 上限）
fn i32_len(len: usize) -> Result<i32, Box<dyn Error>> {
    Ok(i32::try_from(len)?)
}

/// xorshift 伪随机字节（不可压缩，确保 scratch 真实扩张）
fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}

/// 简易校验和（完整性闸门用，首/尾各 4 KiB 的异或折叠）
fn checksum(data: &[u8]) -> u64 {
    let head = data.len().min(4096);
    let tail_start = data.len().saturating_sub(4096);
    let mut acc = 0u64;
    for (i, b) in data[..head].iter().enumerate() {
        acc ^= u64::from(*b) << (i % 8 * 8);
    }
    for (i, b) in data[tail_start..].iter().enumerate() {
        acc = acc.rotate_left(7) ^ (u64::from(*b) << (i % 8 * 8));
    }
    acc
}

/// 标准 LEB128 `VarInt` 写入
fn write_var_int(out: &mut Vec<u8>, value: i32) -> Result<(), Box<dyn Error>> {
    let mut v = u32::try_from(value)?;
    loop {
        let mut byte = u8::try_from(v & 0x7F)?;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            return Ok(());
        }
    }
}

/// `VarInt` 编码后的字节数
fn var_int_len(value: i32) -> Result<usize, Box<dyn Error>> {
    let mut v = u32::try_from(value)?;
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    Ok(n)
}

/// 从字节流头部读取一个 `VarInt` 并使流前进
fn read_var_int(stream: &mut &[u8]) -> Result<i32, Box<dyn Error>> {
    let mut value = 0u32;
    for i in 0..5 {
        let byte = stream[0];
        *stream = &stream[1..];
        value |= u32::from(byte & 0x7F) << (i * 7);
        if byte & 0x80 == 0 {
            return Ok(i32::try_from(value)?);
        }
    }
    Err("基准流中的 varint 未终结".into())
}

/// zlib 压缩（构造入站帧用，与生产解码端格式配对）
fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(COMPRESSION_LEVEL));
    enc.write_all(data)?;
    Ok(enc.finish()?)
}

/// 构造一帧服务器可读的入站数据包（压缩模式下含 `data_length` 字段）
fn build_inbound_frame(id: i32, payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut body = Vec::with_capacity(payload.len() + 5);
    write_var_int(&mut body, id)?;
    body.extend_from_slice(payload);

    let mut inner = Vec::new();
    if body.len() >= COMPRESSION_THRESHOLD {
        let compressed = zlib_compress(&body)?;
        write_var_int(&mut inner, i32_len(body.len())?)?;
        inner.extend_from_slice(&compressed);
    } else {
        write_var_int(&mut inner, 0)?;
        inner.extend_from_slice(&body);
    }

    let mut out = Vec::with_capacity(inner.len() + 5);
    write_var_int(&mut out, i32_len(inner.len())?)?;
    out.extend_from_slice(&inner);
    Ok(out)
}

/// 当前进程 RSS（字节）
fn current_rss() -> u64 {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let _ = sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map_or(0, sysinfo::Process::memory)
}

const fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

/// 计数写端：丢弃字节只记账（两模式输出总字节数交叉校验用）
#[derive(Clone, Default)]
struct CountingSink {
    bytes: Arc<AtomicUsize>,
}

impl AsyncWrite for CountingSink {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.bytes.fetch_add(buf.len(), Ordering::Relaxed);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// 0.3.14 旧编码路径复刻：压缩/组帧 scratch 只增不减。
struct LegacyEncoder {
    compression_scratch: Vec<u8>,
    frame_scratch: Vec<u8>,
    compressor: Compress,
}

impl LegacyEncoder {
    fn new() -> Self {
        Self {
            compression_scratch: Vec::new(),
            frame_scratch: Vec::new(),
            compressor: Compress::new(Compression::new(COMPRESSION_LEVEL), true),
        }
    }

    /// 复刻旧 `write_packet`/`frame_packet` 的容量行为与线上字节
    fn write_packet(
        &mut self,
        packet_data: &[u8],
        wire_bytes: &AtomicUsize,
    ) -> Result<(), Box<dyn Error>> {
        self.frame_scratch.clear();
        if packet_data.len() >= COMPRESSION_THRESHOLD {
            self.compression_scratch.clear();
            let hint = packet_data.len() + packet_data.len() / 16 + 64;
            let cap = self.compression_scratch.capacity();
            if hint > cap {
                self.compression_scratch.reserve(hint - cap);
            }
            self.compressor.reset();
            let status = self.compressor.compress_vec(
                packet_data,
                &mut self.compression_scratch,
                FlushCompress::Finish,
            )?;
            assert!(matches!(status, Status::StreamEnd));

            let data_len = i32_len(packet_data.len())?;
            let full = var_int_len(data_len)? + self.compression_scratch.len();
            let header = var_int_len(i32_len(full)?)? + var_int_len(data_len)?;
            let need = header + self.compression_scratch.len();
            let fcap = self.frame_scratch.capacity();
            if need > fcap {
                self.frame_scratch.reserve(need - fcap);
            }
            write_var_int(&mut self.frame_scratch, i32_len(full)?)?;
            write_var_int(&mut self.frame_scratch, data_len)?;
            let comp = std::mem::take(&mut self.compression_scratch);
            self.frame_scratch.extend_from_slice(&comp);
            self.compression_scratch = comp;
        } else {
            let full = 1 + packet_data.len();
            let need = var_int_len(i32_len(full)?)? + full;
            let fcap = self.frame_scratch.capacity();
            if need > fcap {
                self.frame_scratch.reserve(need - fcap);
            }
            write_var_int(&mut self.frame_scratch, i32_len(full)?)?;
            write_var_int(&mut self.frame_scratch, 0)?;
            self.frame_scratch.extend_from_slice(packet_data);
        }
        wire_bytes.fetch_add(self.frame_scratch.len(), Ordering::Relaxed);
        Ok(())
    }

    const fn scratch_bytes(&self) -> usize {
        self.compression_scratch.capacity() + self.frame_scratch.capacity()
    }
}

/// 0.3.14 旧解码路径复刻：负载 scratch 与冻结负载共享底层
/// 分配，读毕绝不换新（空闲心跳期大包分配随之驻留）。
struct LegacyDecoder {
    scratch: BytesMut,
    stream: &'static [u8],
}

impl LegacyDecoder {
    fn new(stream: &'static [u8]) -> Self {
        Self {
            scratch: BytesMut::new(),
            stream,
        }
    }

    fn read_packet(&mut self) -> Result<Bytes, Box<dyn Error>> {
        let packet_len = usize::try_from(read_var_int(&mut self.stream)?)?;
        let (frame, rest) = self.stream.split_at(packet_len);
        self.stream = rest;

        let mut frame = frame;
        let data_len = usize::try_from(read_var_int(&mut frame)?)?;
        let decompressed = if data_len > 0 {
            let mut v = Vec::with_capacity(data_len);
            flate2::read::ZlibDecoder::new(frame).read_to_end(&mut v)?;
            v
        } else {
            frame.to_vec()
        };

        let mut cursor = &decompressed[..];
        let _id = read_var_int(&mut cursor)?;
        let hint = cursor.len();
        self.scratch.clear();
        self.scratch.reserve(hint);
        self.scratch.extend_from_slice(cursor);
        Ok(self.scratch.split_to(self.scratch.len()).freeze())
    }

    /// 大包负载 drop 后，scratch 若仍引用其分配则返回分配大小
    fn retained_from(&self, big_range: (usize, usize)) -> usize {
        let (start, len) = big_range;
        let ptr = self.scratch.as_ptr() as usize;
        if (start..=start + len).contains(&ptr) {
            len
        } else {
            0
        }
    }
}

/// 子进程入口：跑完整工作负载并输出结果 JSON
async fn run_child(mode: &str) -> Result<Value, Box<dyn Error>> {
    let rss_baseline = current_rss();

    // 共享负载与共享入站流（leak 为 'static，进程退出即回收）
    let big_payload: &'static [u8] =
        Box::leak(pseudo_random_bytes(BIG_PACKET, 0xB16).into_boxed_slice());
    let small_payload: &'static [u8] =
        Box::leak(pseudo_random_bytes(SMALL_PACKET, 0x5A11).into_boxed_slice());
    let medium_payload: &'static [u8] =
        Box::leak(pseudo_random_bytes(MEDIUM_PACKET, 0xED1B).into_boxed_slice());

    let mut stream_bytes = Vec::new();
    stream_bytes.extend_from_slice(&build_inbound_frame(63, big_payload)?);
    for _ in 0..HEARTBEATS {
        stream_bytes.extend_from_slice(&build_inbound_frame(15, &[])?);
    }
    for _ in 0..ROUNDS {
        stream_bytes.extend_from_slice(&build_inbound_frame(31, small_payload)?);
    }
    let stream: &'static [u8] = Box::leak(stream_bytes.into_boxed_slice());

    let big_checksum = checksum(big_payload);

    if mode == "new" {
        run_new(
            stream,
            big_checksum,
            big_payload,
            small_payload,
            medium_payload,
            rss_baseline,
        )
        .await
    } else if mode == "legacy" {
        run_legacy(
            stream,
            big_checksum,
            big_payload,
            small_payload,
            medium_payload,
            rss_baseline,
        )
    } else {
        Err(format!("未知模式：{mode}").into())
    }
}

/// new 模式：当前实现（真实编解码器）
async fn run_new(
    stream: &'static [u8],
    big_checksum: u64,
    big_payload: &'static [u8],
    small_payload: &'static [u8],
    medium_payload: &'static [u8],
    rss_baseline: u64,
) -> Result<Value, Box<dyn Error>> {
    let wire_total = Arc::new(AtomicUsize::new(0));
    let mut encoders: Vec<TCPNetworkEncoder<CountingSink>> = (0..CONNECTIONS)
        .map(|_| {
            let mut enc = TCPNetworkEncoder::new(CountingSink {
                bytes: wire_total.clone(),
            });
            enc.set_compression((COMPRESSION_THRESHOLD, COMPRESSION_LEVEL));
            enc
        })
        .collect();
    let mut decoders: Vec<TCPNetworkDecoder<&'static [u8]>> = (0..CONNECTIONS)
        .map(|_| {
            let mut dec = TCPNetworkDecoder::new(stream);
            dec.set_compression(COMPRESSION_THRESHOLD);
            dec
        })
        .collect();

    // 阶段 1：登录突发（每连接大包出入各一）
    for (enc, dec) in encoders.iter_mut().zip(decoders.iter_mut()) {
        enc.write_packet(Bytes::from_static(big_payload)).await?;
        enc.flush().await?;
        let pkt = dec.get_raw_packet().await?;
        assert_eq!(pkt.payload.len(), BIG_PACKET, "大包负载长度应一致");
        assert_eq!(checksum(&pkt.payload), big_checksum, "大包内容应一致");
    }

    // 阶段 1.5：空闲窗口（零负载心跳）
    for dec in &mut decoders {
        for _ in 0..HEARTBEATS {
            let pkt = dec.get_raw_packet().await?;
            assert!(pkt.payload.is_empty(), "心跳应无负载");
        }
    }

    // 检查点 A：空闲窗口末端的留存账
    let enc_scratch_a: usize = encoders
        .iter()
        .map(|enc| {
            let (comp, frame) = enc.scratch_capacity();
            comp + frame
        })
        .sum();
    let dec_retained_a: usize = decoders
        .iter()
        .map(TCPNetworkDecoder::payload_scratch_capacity)
        .sum();
    let rss_a = current_rss();

    // 阶段 2：稳态游玩（计时）
    let start = Instant::now();
    let mut packets = 0u64;
    for round in 0..ROUNDS {
        for (enc, dec) in encoders.iter_mut().zip(decoders.iter_mut()) {
            enc.write_packet(Bytes::from_static(small_payload)).await?;
            let pkt = dec.get_raw_packet().await?;
            if round % 100 == 0 {
                assert_eq!(&pkt.payload[..], small_payload, "小包内容应一致");
            }
            packets += 2;
            if round % MEDIUM_EVERY == 0 {
                enc.write_packet(Bytes::from_static(medium_payload)).await?;
                packets += 1;
            }
        }
    }
    for enc in &mut encoders {
        enc.flush().await?;
    }
    let phase2 = start.elapsed();

    // 检查点 B：稳态末端的留存账
    let enc_scratch_b: usize = encoders
        .iter()
        .map(|enc| {
            let (comp, frame) = enc.scratch_capacity();
            comp + frame
        })
        .sum();
    let dec_retained_b: usize = decoders
        .iter()
        .map(TCPNetworkDecoder::payload_scratch_capacity)
        .sum();
    let rss_b = current_rss();

    Ok(json!({
        "mode": "new",
        "connections": CONNECTIONS,
        "rss_baseline_mib": mib(rss_baseline),
        "checkpoint_a": {
            "rss_mib": mib(rss_a),
            "encoder_scratch_mib": mib(enc_scratch_a as u64),
            "decoder_retained_mib": mib(dec_retained_a as u64),
        },
        "checkpoint_b": {
            "rss_mib": mib(rss_b),
            "encoder_scratch_mib": mib(enc_scratch_b as u64),
            "decoder_retained_mib": mib(dec_retained_b as u64),
        },
        "wire_bytes": wire_total.load(Ordering::Relaxed),
        "phase2_ms": u64::try_from(phase2.as_millis()).unwrap_or(u64::MAX),
        "phase2_packets": packets,
        "throughput_packets_per_sec": packets as f64 / phase2.as_secs_f64(),
        "integrity_ok": true,
    }))
}

/// legacy 模式：0.3.14 旧实现复刻（同步路径，无网络等待）
fn run_legacy(
    stream: &'static [u8],
    big_checksum: u64,
    big_payload: &'static [u8],
    small_payload: &'static [u8],
    medium_payload: &'static [u8],
    rss_baseline: u64,
) -> Result<Value, Box<dyn Error>> {
    let wire_total = Arc::new(AtomicUsize::new(0));
    let mut encoders: Vec<LegacyEncoder> = (0..CONNECTIONS).map(|_| LegacyEncoder::new()).collect();
    let mut decoders: Vec<LegacyDecoder> = (0..CONNECTIONS)
        .map(|_| LegacyDecoder::new(stream))
        .collect();
    let mut big_ranges = Vec::with_capacity(CONNECTIONS);

    // 阶段 1：登录突发（大包负载消费后即 drop，模拟入队被取走）
    for (enc, dec) in encoders.iter_mut().zip(decoders.iter_mut()) {
        enc.write_packet(big_payload, &wire_total)?;
        let pkt = dec.read_packet()?;
        assert_eq!(pkt.len(), BIG_PACKET, "大包负载长度应一致");
        assert_eq!(checksum(&pkt), big_checksum, "大包内容应一致");
        big_ranges.push((pkt.as_ptr() as usize, pkt.len()));
        drop(pkt);
    }

    // 阶段 1.5：空闲窗口（零负载心跳不触发 reserve，旧实现
    // 在此窗口持续抱住大包分配）
    for dec in &mut decoders {
        for _ in 0..HEARTBEATS {
            let pkt = dec.read_packet()?;
            assert!(pkt.is_empty(), "心跳应无负载");
            drop(pkt);
        }
    }

    // 检查点 A
    let enc_scratch_a: usize = encoders.iter().map(LegacyEncoder::scratch_bytes).sum();
    let dec_retained_a: usize = decoders
        .iter()
        .zip(&big_ranges)
        .map(|(dec, &range)| dec.retained_from(range))
        .sum();
    let rss_a = current_rss();

    // 阶段 2：稳态游玩（计时）
    let start = Instant::now();
    let mut packets = 0u64;
    for round in 0..ROUNDS {
        for (enc, dec) in encoders.iter_mut().zip(decoders.iter_mut()) {
            enc.write_packet(small_payload, &wire_total)?;
            let pkt = dec.read_packet()?;
            if round % 100 == 0 {
                assert_eq!(&pkt[..], small_payload, "小包内容应一致");
            }
            drop(pkt);
            packets += 2;
            if round % MEDIUM_EVERY == 0 {
                enc.write_packet(medium_payload, &wire_total)?;
                packets += 1;
            }
        }
    }
    let phase2 = start.elapsed();

    // 检查点 B
    let enc_scratch_b: usize = encoders.iter().map(LegacyEncoder::scratch_bytes).sum();
    let dec_retained_b: usize = decoders
        .iter()
        .zip(&big_ranges)
        .map(|(dec, &range)| dec.retained_from(range))
        .sum();
    let rss_b = current_rss();

    Ok(json!({
        "mode": "legacy",
        "connections": CONNECTIONS,
        "rss_baseline_mib": mib(rss_baseline),
        "checkpoint_a": {
            "rss_mib": mib(rss_a),
            "encoder_scratch_mib": mib(enc_scratch_a as u64),
            "decoder_retained_mib": mib(dec_retained_a as u64),
        },
        "checkpoint_b": {
            "rss_mib": mib(rss_b),
            "encoder_scratch_mib": mib(enc_scratch_b as u64),
            "decoder_retained_mib": mib(dec_retained_b as u64),
        },
        "wire_bytes": wire_total.load(Ordering::Relaxed),
        "phase2_ms": u64::try_from(phase2.as_millis()).unwrap_or(u64::MAX),
        "phase2_packets": packets,
        "throughput_packets_per_sec": packets as f64 / phase2.as_secs_f64(),
        "integrity_ok": true,
    }))
}

/// 从子进程 JSON 中取数（缺字段视为基准失败）
fn get_f64(v: &Value, path: &[&str]) -> Result<f64, Box<dyn Error>> {
    let mut cur = v;
    for key in path {
        cur = &cur[*key];
    }
    cur.as_f64().ok_or("JSON 数值字段缺失".into())
}

fn run_parent() -> Result<(), Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let mut results = Vec::new();
    for mode in ["new", "legacy"] {
        println!("…… 运行子进程：{mode}");
        let out = Command::new(&exe).arg("--child").arg(mode).output()?;
        assert!(
            out.status.success(),
            "子进程 {mode} 失败：{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout)?;
        let line = stdout.lines().last().ok_or("子进程应有输出")?;
        results.push(serde_json::from_str::<Value>(line)?);
    }

    let new = &results[0];
    let legacy = &results[1];

    // 线上字节闸门：两模式编码输出总字节数必须完全相等
    let wire_new = new["wire_bytes"].as_u64().ok_or("应有 wire_bytes")?;
    let wire_legacy = legacy["wire_bytes"].as_u64().ok_or("应有 wire_bytes")?;
    assert_eq!(
        wire_new, wire_legacy,
        "两模式线上字节数必须相等（{wire_new} vs {wire_legacy}）"
    );

    println!();
    println!(
        "| 模式 | A: RSS | A: 编码 scratch | A: 解码驻留 | B: RSS | B: 编码 scratch | B: 解码驻留 | 稳态吞吐 (pkt/s) |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    for r in [&legacy, &new] {
        let mode = r["mode"].as_str().ok_or("应有 mode")?;
        let rss_a = get_f64(r, &["checkpoint_a", "rss_mib"])?;
        let enc_a = get_f64(r, &["checkpoint_a", "encoder_scratch_mib"])?;
        let dec_a = get_f64(r, &["checkpoint_a", "decoder_retained_mib"])?;
        let rss_b = get_f64(r, &["checkpoint_b", "rss_mib"])?;
        let enc_b = get_f64(r, &["checkpoint_b", "encoder_scratch_mib"])?;
        let dec_b = get_f64(r, &["checkpoint_b", "decoder_retained_mib"])?;
        let throughput = get_f64(r, &["throughput_packets_per_sec"])?;
        println!(
            "| {mode} | {rss_a:.1} MiB | {enc_a:.1} MiB | {dec_a:.1} MiB | {rss_b:.1} MiB | {enc_b:.1} MiB | {dec_b:.1} MiB | {throughput:.0} |"
        );
    }

    let report = json!({
        "round": 2,
        "subject": "每连接收发 scratch 容量治理（256 KiB 留存上限）",
        "workload": {
            "connections": CONNECTIONS,
            "big_packet_bytes": BIG_PACKET,
            "heartbeats": HEARTBEATS,
            "rounds": ROUNDS,
            "small_packet_bytes": SMALL_PACKET,
            "medium_packet_bytes": MEDIUM_PACKET,
            "medium_every": MEDIUM_EVERY,
            "compression_threshold": COMPRESSION_THRESHOLD,
        },
        "wire_bytes_equal": true,
        "legacy": legacy,
        "new": new,
    });
    let path = "note/report/perf/round2-connection-scratch.json";
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    println!();
    println!("对比 JSON 已写入 {path}");
    Ok(())
}
