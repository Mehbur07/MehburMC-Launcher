//! Minimal uncompressed NBT reader/writer (big-endian, Java edition), enough
//! to round-trip `servers.dat` without losing fields we do not know about.

/// Nesting limit; the game itself stops at 512.
const MAX_DEPTH: usize = 512;
/// Upper bound for array/list lengths read from a file.
const MAX_LEN: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// Element type id + elements (type is kept for empty lists).
    List(u8, Vec<Tag>),
    /// Ordered, so writing keeps the original layout.
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::ByteArray(_) => 7,
            Self::String(_) => 8,
            Self::List(..) => 9,
            Self::Compound(_) => 10,
            Self::IntArray(_) => 11,
            Self::LongArray(_) => 12,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Self::Compound(c) => c.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Tag> {
        match self {
            Self::Compound(c) => c.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_byte(&self) -> Option<i8> {
        match self {
            Self::Byte(b) => Some(*b),
            _ => None,
        }
    }
}

/// Reads a root tag: `(name, compound)`.
pub fn read(bytes: &[u8]) -> Result<(String, Tag), String> {
    let mut r = Reader { buf: bytes, pos: 0 };
    let id = r.u8()?;
    if id != 10 {
        return Err(format!("root tag is {id}, expected a compound"));
    }
    let name = r.string()?;
    let tag = r.payload(id, 0)?;
    Ok((name, tag))
}

/// Writes a named root tag.
pub fn write(name: &str, tag: &Tag) -> Vec<u8> {
    let mut out = vec![tag.id()];
    put_string(&mut out, name);
    put_payload(&mut out, tag);
    out
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or("unexpected end of data")?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }

    fn len(&mut self) -> Result<usize, String> {
        let n = i32::from_be_bytes(self.arr()?);
        usize::try_from(n)
            .ok()
            .filter(|n| *n <= MAX_LEN)
            .ok_or_else(|| format!("invalid length {n}"))
    }

    fn string(&mut self) -> Result<String, String> {
        let n = u16::from_be_bytes(self.arr()?) as usize;
        Ok(decode_mutf8(self.take(n)?))
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag, String> {
        if depth > MAX_DEPTH {
            return Err("nesting too deep".into());
        }
        Ok(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.arr()?)),
            3 => Tag::Int(i32::from_be_bytes(self.arr()?)),
            4 => Tag::Long(i64::from_be_bytes(self.arr()?)),
            5 => Tag::Float(f32::from_be_bytes(self.arr()?)),
            6 => Tag::Double(f64::from_be_bytes(self.arr()?)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|b| *b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.len()?;
                if n > 0 && elem == 0 {
                    return Err("non-empty list of end tags".into());
                }
                let mut items = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    items.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(elem, items)
            }
            10 => {
                let mut fields = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    fields.push((name, self.payload(t, depth + 1)?));
                }
                Tag::Compound(fields)
            }
            11 => {
                let n = self.len()?;
                let raw = self.take(n.checked_mul(4).ok_or("invalid length")?)?;
                Tag::IntArray(
                    raw.as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| i32::from_be_bytes(*c))
                        .collect(),
                )
            }
            12 => {
                let n = self.len()?;
                let raw = self.take(n.checked_mul(8).ok_or("invalid length")?)?;
                Tag::LongArray(
                    raw.as_chunks::<8>()
                        .0
                        .iter()
                        .map(|c| i64::from_be_bytes(*c))
                        .collect(),
                )
            }
            other => return Err(format!("unknown tag type {other}")),
        })
    }
}

fn put_len(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n.min(i32::MAX as usize) as i32).to_be_bytes());
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    let mut bytes = encode_mutf8(s);
    // Strings are limited to 65535 encoded bytes; cut on a char boundary.
    if bytes.len() > u16::MAX as usize {
        let mut cut = String::new();
        for c in s.chars() {
            cut.push(c);
            if encode_mutf8(&cut).len() > u16::MAX as usize {
                cut.pop();
                break;
            }
        }
        bytes = encode_mutf8(&cut);
    }
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(&bytes);
}

fn put_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::ByteArray(v) => {
            put_len(out, v.len());
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => put_string(out, s),
        Tag::List(elem, items) => {
            // The element type must match the items; fall back to their id.
            let id = items.first().map_or(*elem, Tag::id);
            out.push(id);
            put_len(out, items.len());
            for i in items {
                put_payload(out, i);
            }
        }
        Tag::Compound(fields) => {
            for (k, v) in fields {
                out.push(v.id());
                put_string(out, k);
                put_payload(out, v);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            put_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            put_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

/// Java "modified UTF-8": U+0000 as two bytes, supplementary characters as
/// surrogate pairs of three bytes each.
fn encode_mutf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            0x0001..=0x007F => out.push(u as u8),
            0x0000 | 0x0080..=0x07FF => {
                out.push(0xC0 | (u >> 6) as u8);
                out.push(0x80 | (u & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (u >> 12) as u8);
                out.push(0x80 | ((u >> 6) & 0x3F) as u8);
                out.push(0x80 | (u & 0x3F) as u8);
            }
        }
    }
    out
}

/// Lenient decoder: malformed sequences become U+FFFD.
fn decode_mutf8(b: &[u8]) -> String {
    let mut units = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let cont = |j: usize| {
            b.get(j)
                .filter(|x| *x & 0xC0 == 0x80)
                .map(|x| u16::from(x & 0x3F))
        };
        if c < 0x80 {
            units.push(u16::from(c));
            i += 1;
        } else if c & 0xE0 == 0xC0
            && let Some(x) = cont(i + 1)
        {
            units.push((u16::from(c & 0x1F) << 6) | x);
            i += 2;
        } else if c & 0xF0 == 0xE0
            && let (Some(x), Some(y)) = (cont(i + 1), cont(i + 2))
        {
            units.push((u16::from(c & 0x0F) << 12) | (x << 6) | y);
            i += 3;
        } else {
            units.push(0xFFFD);
            i += 1;
        }
    }
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutf8_round_trip() {
        for s in [
            "",
            "Sunucu",
            "Çılgın şehir ğüö",
            "nul\0byte",
            "emoji 😀 §aRenk",
        ] {
            assert_eq!(decode_mutf8(&encode_mutf8(s)), s);
        }
        assert_eq!(encode_mutf8("\0"), [0xC0, 0x80]);
        // Supplementary characters are surrogate pairs, 6 bytes.
        assert_eq!(encode_mutf8("😀").len(), 6);
    }

    #[test]
    fn round_trip_keeps_every_tag() {
        let tag = Tag::Compound(vec![
            ("b".into(), Tag::Byte(-3)),
            ("s".into(), Tag::Short(300)),
            ("i".into(), Tag::Int(-70000)),
            ("l".into(), Tag::Long(1 << 40)),
            ("f".into(), Tag::Float(1.5)),
            ("d".into(), Tag::Double(-2.25)),
            ("ba".into(), Tag::ByteArray(vec![1, -1])),
            ("str".into(), Tag::String("merhaba".into())),
            ("empty".into(), Tag::List(10, vec![])),
            (
                "list".into(),
                Tag::List(8, vec![Tag::String("a".into()), Tag::String("b".into())]),
            ),
            ("ia".into(), Tag::IntArray(vec![1, 2, 3])),
            ("la".into(), Tag::LongArray(vec![-1])),
            (
                "nested".into(),
                Tag::Compound(vec![("x".into(), Tag::Byte(1))]),
            ),
        ]);
        let bytes = write("", &tag);
        assert_eq!(read(&bytes).unwrap(), (String::new(), tag));
    }

    #[test]
    fn rejects_garbage() {
        assert!(read(&[]).is_err());
        assert!(read(&[8, 0, 0]).is_err());
        // Truncated compound.
        assert!(read(&[10, 0, 0, 8, 0, 1, b'a']).is_err());
        // Huge array length.
        assert!(read(&[10, 0, 0, 7, 0, 1, b'a', 0x7f, 0xff, 0xff, 0xff]).is_err());
        // Deep nesting does not overflow the stack.
        let mut deep = vec![10, 0, 0];
        for _ in 0..2000 {
            deep.extend_from_slice(&[10, 0, 0]);
        }
        assert!(read(&deep).is_err());
    }
}
