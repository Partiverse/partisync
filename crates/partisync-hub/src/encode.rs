//! entry 行紧凑二进制编码（SPEC M3-WP01 §1）——非 JSON，≤300B/条目预算。
//!
//! 键 = entry_id 16B 二进制。值布局：parent 16B、kind 1B、
//! name（varint len + bytes）、content（1B 标记 + 32B）、size（varint）、
//! mtime_ns 8B、flags 2B；varint 编码为 LEB128（无符号）。

/// kind 常量（对齐设备端 schema `entry.kind`：0=file, 1=dir）。
/// hub 为投影层，未知 kind 原样透传（0xFF = 墓碑无来源信息时使用）。
pub const KIND_FILE: u8 = 0;
/// 目录 kind。
pub const KIND_DIR: u8 = 1;

/// flags bit 0：墓碑（删除标记，保留至 WP03 对账窗口——SPEC §1）。
pub const FLAG_DELETED: u16 = 0x0001;

/// entry 平面权威行（M3-WP01 §1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    /// 平面 ID（16B ULID 原生字节）。
    pub entry_id: [u8; 16],
    /// 父目录 ID；`None` = 根。
    pub parent_id: Option<[u8; 16]>,
    /// kind（`KIND_FILE` / `KIND_DIR`，未知值透传）。
    pub kind: u8,
    /// 条目名（单路径段，不含分隔符）。
    pub name: String,
    /// 内容身份（blake3 hex 的 32B 原生形式）；`None` = 目录或未落内容。
    pub content_id: Option<[u8; 32]>,
    /// 逻辑大小（字节）。
    pub size: u64,
    /// 修改时间（纳秒）。
    pub mtime_ns: i64,
    /// 标志位（`FLAG_DELETED` = 墓碑）。
    pub flags: u16,
}

impl EntryRow {
    /// 是否墓碑（删除标记）。
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.flags & FLAG_DELETED != 0
    }
}

/// 编解码错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// 值字节流意外截断。
    UnexpectedEof,
    /// varint 超过 10 字节（u64 上限）。
    VarintOverflow,
    /// varint 编码的名称长度超界（> u32 上限或超出剩余字节）。
    InvalidNameLength,
    /// content 标记字节非法（非 0x00/0x01）。
    InvalidContentMarker,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "entry row encoding: unexpected eof"),
            Self::VarintOverflow => write!(f, "entry row encoding: varint overflow"),
            Self::InvalidNameLength => write!(f, "entry row encoding: invalid name length"),
            Self::InvalidContentMarker => write!(f, "entry row encoding: invalid content marker"),
        }
    }
}

impl std::error::Error for EncodeError {}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64, EncodeError> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *buf.get(*pos).ok_or(EncodeError::UnexpectedEof)?;
        *pos += 1;
        if shift >= 64 {
            return Err(EncodeError::VarintOverflow);
        }
        result |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
    }
}

/// 编码 entry 行（值部分；键为 `entry_id` 原生 16B，调用方持有）。
///
/// # Errors
/// 名称长度超 `u32` 上限时返回 [`EncodeError::InvalidNameLength`]。
pub fn encode_entry_row(row: &EntryRow) -> Result<Vec<u8>, EncodeError> {
    let name = row.name.as_bytes();
    if name.len() > u32::MAX as usize {
        return Err(EncodeError::InvalidNameLength);
    }
    let mut out = Vec::with_capacity(16 + 1 + 5 + name.len() + 33 + 10 + 8 + 2);
    // parent：16B，全零 = None（root）
    out.extend_from_slice(row.parent_id.as_ref().unwrap_or(&[0u8; 16]));
    out.push(row.kind);
    write_varint(&mut out, name.len() as u64);
    out.extend_from_slice(name);
    // content：1B 标记 + 32B
    match row.content_id {
        None => out.push(0x00),
        Some(cid) => {
            out.push(0x01);
            out.extend_from_slice(&cid);
        }
    }
    write_varint(&mut out, row.size);
    out.extend_from_slice(&row.mtime_ns.to_be_bytes());
    out.extend_from_slice(&row.flags.to_be_bytes());
    Ok(out)
}

/// 解码 entry 行。
///
/// # Errors
/// 字节流截断或标记非法时返回对应 [`EncodeError`]。
pub fn decode_entry_row(buf: &[u8]) -> Result<EntryRow, EncodeError> {
    if buf.len() < 16 + 1 {
        return Err(EncodeError::UnexpectedEof);
    }
    let mut parent = [0u8; 16];
    parent.copy_from_slice(&buf[0..16]);
    let parent_id = if parent == [0u8; 16] {
        None
    } else {
        Some(parent)
    };
    let kind = buf[16];
    let mut pos = 17;
    let name_len = read_varint(buf, &mut pos)? as usize;
    if buf.len() < pos + name_len {
        return Err(EncodeError::UnexpectedEof);
    }
    let name = std::str::from_utf8(&buf[pos..pos + name_len])
        .map_err(|_| EncodeError::InvalidNameLength)?;
    pos += name_len;
    let marker = *buf.get(pos).ok_or(EncodeError::UnexpectedEof)?;
    pos += 1;
    let content_id = match marker {
        0x00 => None,
        0x01 => {
            if buf.len() < pos + 32 {
                return Err(EncodeError::UnexpectedEof);
            }
            let mut cid = [0u8; 32];
            cid.copy_from_slice(&buf[pos..pos + 32]);
            pos += 32;
            Some(cid)
        }
        _ => return Err(EncodeError::InvalidContentMarker),
    };
    let size = read_varint(buf, &mut pos)?;
    if buf.len() < pos + 8 + 2 {
        return Err(EncodeError::UnexpectedEof);
    }
    let mtime_ns = i64::from_be_bytes(buf[pos..pos + 8].try_into().expect("8 bytes"));
    pos += 8;
    let flags = u16::from_be_bytes(buf[pos..pos + 2].try_into().expect("2 bytes"));
    Ok(EntryRow {
        entry_id: [0u8; 16], // 键即 id，值不冗余；调用方按需回填
        parent_id,
        kind,
        name: name.to_owned(),
        content_id,
        size,
        mtime_ns,
        flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> EntryRow {
        EntryRow {
            entry_id: [7u8; 16],
            parent_id: Some([1u8; 16]),
            kind: KIND_FILE,
            name: "report.pdf".into(),
            content_id: Some([9u8; 32]),
            size: 123_456,
            mtime_ns: 1_726_800_000_000_000_000,
            flags: 0,
        }
    }

    #[test]
    fn roundtrip_sample() {
        let row = sample_row();
        let buf = encode_entry_row(&row).expect("encode");
        let back = decode_entry_row(&buf).expect("decode");
        assert_eq!(back.parent_id, row.parent_id);
        assert_eq!(back.kind, row.kind);
        assert_eq!(back.name, row.name);
        assert_eq!(back.content_id, row.content_id);
        assert_eq!(back.size, row.size);
        assert_eq!(back.mtime_ns, row.mtime_ns);
        assert_eq!(back.flags, row.flags);
    }

    #[test]
    fn budget_typical_row_under_300b() {
        // 键 16B + 值；名称 ≤32B 的常规行必须 ≤300B（SPEC §1 预算）
        let mut row = sample_row();
        row.name = "a-typical-asset-name.bin".into();
        let buf = encode_entry_row(&row).expect("encode");
        let total = 16 + buf.len();
        assert!(total <= 300, "row budget exceeded: {total}B");
    }

    #[test]
    fn root_row_no_parent_no_content() {
        let row = EntryRow {
            entry_id: [1u8; 16],
            parent_id: None,
            kind: KIND_DIR,
            name: String::new(),
            content_id: None,
            size: 0,
            mtime_ns: 0,
            flags: FLAG_DELETED,
        };
        let buf = encode_entry_row(&row).expect("encode");
        let back = decode_entry_row(&buf).expect("decode");
        assert_eq!(back.parent_id, None);
        assert_eq!(back.kind, KIND_DIR);
        assert!(back.is_deleted());
    }

    #[test]
    fn truncated_input_rejected() {
        let buf = encode_entry_row(&sample_row()).expect("encode");
        for cut in [0usize, 5, 17, buf.len() - 3] {
            assert_eq!(
                decode_entry_row(&buf[..cut]),
                Err(EncodeError::UnexpectedEof)
            );
        }
    }
}
