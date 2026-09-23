//! A minimal, read-only ZIP reader for upstream `.proj` archives: the central directory, stored
//! and deflated entries (RFC 1951), and a CRC-32 check of every entry read.
//!
//! **A stopgap.** The crate has no zip dependency; this reader exists so that `.proj` import works
//! today, and should give way to a maintained crate. It reads what upstream's GUI writes
//! (wxZipOutputStream: deflate, no encryption, no ZIP64) and refuses everything else by name:
//! encryption, ZIP64, multi-disk archives, and compression methods other than 0 and 8.
//!
//! It never writes to disk, so entry names cannot escape anywhere (no zip-slip); they are only
//! keys. An entry is inflated only when asked for, into at most the size the central directory
//! declares, which is itself capped at the deflate format's maximum ratio to the compressed size,
//! so a small archive cannot make it allocate much. The inflated bytes must have exactly the
//! declared size and CRC-32.

use super::{ImportError, Result};

const FMT: &str = "zip";

/// Deflate cannot expand data by more than this factor (a length-258 match costs at least one
/// bit plus the one-bit distance code, so about 1032:1); a larger declared size is a lie.
const MAX_DEFLATE_RATIO: u64 = 1032;

/// One entry of the central directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The name, with `\` turned into `/`.
    pub name: String,
    /// 0 stored, 8 deflated.
    pub method: u16,
    pub flags: u16,
    pub crc32: u32,
    pub compressed_size: u64,
    pub size: u64,
    local_header: u64,
}

/// A parsed archive, borrowing its bytes.
#[derive(Debug)]
pub struct Archive<'a> {
    bytes: &'a [u8],
    entries: Vec<Entry>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

impl<'a> Archive<'a> {
    /// Reads the central directory.
    pub fn parse(bytes: &'a [u8]) -> Result<Archive<'a>> {
        // The end-of-central-directory record: 22 bytes plus a comment of up to 65,535, found by
        // searching backwards for a signature whose comment length reaches exactly the end.
        let min = bytes.len().saturating_sub(22 + 65_535);
        let mut eocd = None;
        let mut at = bytes.len().checked_sub(22);
        while let Some(i) = at {
            if i < min {
                break;
            }
            if u32_at(bytes, i) == Some(0x0605_4b50)
                && u16_at(bytes, i + 20).map(|c| i + 22 + c as usize) == Some(bytes.len())
            {
                eocd = Some(i);
                break;
            }
            at = i.checked_sub(1);
        }
        let e = eocd.ok_or_else(|| {
            ImportError::invalid(FMT, "no end-of-central-directory record: not a zip archive")
        })?;
        let disk = u16_at(bytes, e + 4).unwrap_or(0);
        let cd_disk = u16_at(bytes, e + 6).unwrap_or(0);
        let on_disk = u16_at(bytes, e + 8).unwrap_or(0);
        let total = u16_at(bytes, e + 10).unwrap_or(0);
        let cd_size = u32_at(bytes, e + 12).unwrap_or(0);
        let cd_offset = u32_at(bytes, e + 16).unwrap_or(0);
        if total == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
            return Err(ImportError::unsupported(FMT, "ZIP64 archives"));
        }
        if disk != 0 || cd_disk != 0 || on_disk != total {
            return Err(ImportError::unsupported(FMT, "multi-disk archives"));
        }
        let cd_start = cd_offset as usize;
        let cd_end = cd_start
            .checked_add(cd_size as usize)
            .filter(|&end| end <= e)
            .ok_or_else(|| {
                ImportError::invalid(FMT, "the central directory lies outside the file")
            })?;
        let cd = &bytes[cd_start..cd_end];
        // Each record takes at least 46 bytes; the count cannot exceed what is there.
        if (total as usize) * 46 > cd.len() {
            return Err(ImportError::truncated(
                FMT,
                format!("the {total} central-directory records"),
            ));
        }
        let mut entries = Vec::with_capacity(total as usize);
        let mut p = 0usize;
        for i in 0..total {
            let bad = || ImportError::truncated(FMT, format!("central-directory record {i}"));
            if u32_at(cd, p) != Some(0x0201_4b50) {
                return Err(ImportError::invalid(
                    FMT,
                    format!("central-directory record {i} has no signature"),
                ));
            }
            let flags = u16_at(cd, p + 8).ok_or_else(bad)?;
            let method = u16_at(cd, p + 10).ok_or_else(bad)?;
            let crc32 = u32_at(cd, p + 16).ok_or_else(bad)?;
            let csize = u32_at(cd, p + 20).ok_or_else(bad)?;
            let size = u32_at(cd, p + 24).ok_or_else(bad)?;
            let name_len = u16_at(cd, p + 28).ok_or_else(bad)? as usize;
            let extra_len = u16_at(cd, p + 30).ok_or_else(bad)? as usize;
            let comment_len = u16_at(cd, p + 32).ok_or_else(bad)? as usize;
            let local = u32_at(cd, p + 42).ok_or_else(bad)?;
            let name = cd.get(p + 46..p + 46 + name_len).ok_or_else(bad)?;
            if csize == 0xFFFF_FFFF || size == 0xFFFF_FFFF || local == 0xFFFF_FFFF {
                return Err(ImportError::unsupported(FMT, "ZIP64 entries"));
            }
            entries.push(Entry {
                name: String::from_utf8_lossy(name).replace('\\', "/"),
                method,
                flags,
                crc32,
                compressed_size: u64::from(csize),
                size: u64::from(size),
                local_header: u64::from(local),
            });
            p += 46 + name_len + extra_len + comment_len;
        }
        Ok(Archive { bytes, entries })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The one entry with this exact name. Two entries with one name is an error, since which one
    /// a reader would take is arbitrary.
    pub fn find(&self, name: &str) -> Result<Option<&Entry>> {
        let mut found = self.entries.iter().filter(|e| e.name == name);
        let first = found.next();
        if found.next().is_some() {
            return Err(ImportError::invalid(
                FMT,
                format!("the archive holds `{name}` twice"),
            ));
        }
        Ok(first)
    }

    /// The contents of the entry with this name.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let entry = self
            .find(name)?
            .ok_or_else(|| ImportError::invalid(FMT, format!("the archive holds no `{name}`")))?;
        self.read_entry(entry)
    }

    /// The contents of an entry, inflated and checked.
    pub fn read_entry(&self, entry: &Entry) -> Result<Vec<u8>> {
        let what = || format!("entry `{}`", entry.name);
        if entry.flags & 1 != 0 {
            return Err(ImportError::unsupported(
                FMT,
                format!("{} is encrypted", what()),
            ));
        }
        let lh = entry.local_header as usize;
        if u32_at(self.bytes, lh) != Some(0x0403_4b50) {
            return Err(ImportError::invalid(
                FMT,
                format!("{} has no local header", what()),
            ));
        }
        let name_len =
            u16_at(self.bytes, lh + 26).ok_or_else(|| ImportError::truncated(FMT, what()))?;
        let extra_len =
            u16_at(self.bytes, lh + 28).ok_or_else(|| ImportError::truncated(FMT, what()))?;
        let start = lh + 30 + name_len as usize + extra_len as usize;
        let data = start
            .checked_add(entry.compressed_size as usize)
            .and_then(|end| self.bytes.get(start..end))
            .ok_or_else(|| ImportError::truncated(FMT, what()))?;
        let out = match entry.method {
            0 => {
                if entry.size != entry.compressed_size {
                    return Err(ImportError::invalid(
                        FMT,
                        format!("{} is stored but its two sizes differ", what()),
                    ));
                }
                data.to_vec()
            }
            8 => {
                let cap = entry
                    .compressed_size
                    .saturating_mul(MAX_DEFLATE_RATIO)
                    .saturating_add(64);
                if entry.size > cap {
                    return Err(ImportError::invalid(
                        FMT,
                        format!(
                            "{} declares {} bytes from {} compressed, beyond deflate's ratio",
                            what(),
                            entry.size,
                            entry.compressed_size
                        ),
                    ));
                }
                let out = inflate(data, entry.size as usize)
                    .map_err(|m| ImportError::invalid(FMT, format!("{}: {m}", what())))?;
                if out.len() as u64 != entry.size {
                    return Err(ImportError::invalid(
                        FMT,
                        format!(
                            "{} inflates to {} bytes, not the {} declared",
                            what(),
                            out.len(),
                            entry.size
                        ),
                    ));
                }
                out
            }
            m => {
                return Err(ImportError::unsupported(
                    FMT,
                    format!("{} uses compression method {m}", what()),
                ));
            }
        };
        let crc = crc32(&out);
        if crc != entry.crc32 {
            return Err(ImportError::invalid(
                FMT,
                format!(
                    "{} fails its CRC-32: {crc:08x}, the directory says {:08x}",
                    what(),
                    entry.crc32
                ),
            ));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------------------
// CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320).

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// The CRC-32 zip uses.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------------------------
// Inflate (RFC 1951), after zlib's `puff.c`: canonical Huffman codes decoded bit by bit.

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    count: u32,
}

type InflateResult<T> = std::result::Result<T, String>;

impl Bits<'_> {
    fn bits(&mut self, n: u32) -> InflateResult<u32> {
        while self.count < n {
            let b = *self
                .data
                .get(self.pos)
                .ok_or_else(|| "the compressed data ends early".to_string())?;
            self.pos += 1;
            self.buf |= u64::from(b) << self.count;
            self.count += 8;
        }
        let v = (self.buf & ((1u64 << n) - 1)) as u32;
        self.buf >>= n;
        self.count -= n;
        Ok(v)
    }
}

/// A canonical Huffman code: how many codes of each length, and the symbols in code order.
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    /// Builds the code from symbol lengths. An over-subscribed set is an error; an incomplete
    /// one is allowed (a missing code is an error only if it is ever read).
    fn new(lengths: &[u8]) -> InflateResult<Huffman> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        let mut left: i32 = 1;
        for &c in &counts[1..] {
            left <<= 1;
            left -= i32::from(c);
            if left < 0 {
                return Err("an over-subscribed Huffman code".to_string());
            }
        }
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits<'_>) -> InflateResult<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.bits(1)? as i32;
            let count = i32::from(self.counts[len]);
            if code - first < count {
                return Ok(self.symbols[(index + code - first) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("an undefined Huffman code".to_string())
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Inflates a raw deflate stream into at most `limit` bytes.
pub fn inflate(data: &[u8], limit: usize) -> InflateResult<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(limit.min(64 << 20));
    let mut bits = Bits {
        data,
        pos: 0,
        buf: 0,
        count: 0,
    };
    loop {
        let last = bits.bits(1)?;
        match bits.bits(2)? {
            0 => {
                // Stored: drop the rest of the current byte.
                bits.buf = 0;
                bits.count = 0;
                let p = bits.pos;
                let hdr = data
                    .get(p..p + 4)
                    .ok_or_else(|| "a stored block header ends early".to_string())?;
                let len = u16::from_le_bytes([hdr[0], hdr[1]]);
                let nlen = u16::from_le_bytes([hdr[2], hdr[3]]);
                if len != !nlen {
                    return Err("a stored block's length check fails".to_string());
                }
                let body = data
                    .get(p + 4..p + 4 + len as usize)
                    .ok_or_else(|| "a stored block ends early".to_string())?;
                if out.len() + body.len() > limit {
                    return Err("the data inflates past its declared size".to_string());
                }
                out.extend_from_slice(body);
                bits.pos = p + 4 + len as usize;
            }
            1 => {
                let mut lengths = [0u8; 288];
                lengths[..144].fill(8);
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                lengths[280..].fill(8);
                let lit = Huffman::new(&lengths)?;
                let dist = Huffman::new(&[5u8; 30])?;
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut bits)?;
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            _ => return Err("an invalid block type".to_string()),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn dynamic_tables(bits: &mut Bits<'_>) -> InflateResult<(Huffman, Huffman)> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err("a dynamic block declares too many codes".to_string());
    }
    let mut cl = [0u8; 19];
    for &i in &ORDER[..ncode] {
        cl[i] = bits.bits(3)? as u8;
    }
    let clcode = Huffman::new(&cl)?;
    let mut lengths = [0u8; 316];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = clcode.decode(bits)?;
        if sym < 16 {
            lengths[i] = sym as u8;
            i += 1;
            continue;
        }
        let (value, repeat) = match sym {
            16 => {
                if i == 0 {
                    return Err("a length repeat with no previous length".to_string());
                }
                (lengths[i - 1], 3 + bits.bits(2)? as usize)
            }
            17 => (0, 3 + bits.bits(3)? as usize),
            _ => (0, 11 + bits.bits(7)? as usize),
        };
        if i + repeat > nlen + ndist {
            return Err("code lengths overrun their table".to_string());
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err("a dynamic block has no end-of-block code".to_string());
    }
    Ok((
        Huffman::new(&lengths[..nlen])?,
        Huffman::new(&lengths[nlen..nlen + ndist])?,
    ))
}

fn codes(
    bits: &mut Bits<'_>,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    limit: usize,
) -> InflateResult<()> {
    loop {
        let sym = lit.decode(bits)? as usize;
        if sym < 256 {
            if out.len() >= limit {
                return Err("the data inflates past its declared size".to_string());
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = sym - 257;
            if s >= 29 {
                return Err("an invalid length symbol".to_string());
            }
            let len = LEN_BASE[s] as usize + bits.bits(u32::from(LEN_EXTRA[s]))? as usize;
            let d = dist.decode(bits)? as usize;
            if d >= 30 {
                return Err("an invalid distance symbol".to_string());
            }
            let back = DIST_BASE[d] as usize + bits.bits(u32::from(DIST_EXTRA[d]))? as usize;
            if back > out.len() {
                return Err("a distance reaches before the start of the data".to_string());
            }
            if out.len() + len > limit {
                return Err("the data inflates past its declared size".to_string());
            }
            let start = out.len() - back;
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_of_the_check_string() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn inflates_stored_fixed_and_dynamic_blocks() {
        // Stored: final block, type 0, "abc".
        let stored = [0x01, 0x03, 0x00, 0xFC, 0xFF, b'a', b'b', b'c'];
        assert_eq!(inflate(&stored, 3).unwrap(), b"abc");
        assert!(inflate(&stored, 2).is_err());
        // zlib.compress(b"hello hello hello hello", 9)[2:-4]: fixed Huffman with a match.
        let fixed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01];
        assert_eq!(inflate(&fixed, 23).unwrap(), b"hello hello hello hello");
        assert!(inflate(&fixed, 22).is_err());
        // Raw deflate at level 9 of 40 numbered lines (zlib, wbits -15): one dynamic block.
        let hex = concat!(
            "95955b56c3300c44ff5985966049b663b31b1e010aa18196d2c2ea795893fff9eeb947d1e87abaecf6b3a46bf9789ae5",
            "fdb4bb7b91dbc37adecbc37a91e7d3ebdb51d6cff9f0fff372f3fd25f7eba3a4abe58f528ed2411947e5413947f54165",
            "f20bebc00a87591958e5308f69131948ecd638ac46909dc35a5c4d4945344112d612c3445214cdb1a13a7bf54854595d",
            "7a5c505961608c56d6344c24a5b1861d496ddc906a67edc67b27cdc930c7487372c644b6625aec68a439c5225523cd29",
            "53dcd148732acc31d29cba4d24cd99b61d4973a62d55b672704727cde930c749733a5c75b673129e87b3a593f0229d6d",
            "1d45097861ab15f6786549549d93faa8a35dbdd17d8e6c4981b4fcfe87fc00",
        );
        let dynamic: Vec<u8> = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
            .collect();
        assert_eq!((dynamic[0] >> 1) & 3, 2, "the vector is a dynamic block");
        let expected: Vec<u8> = (0..40)
            .flat_map(|i| {
                format!(
                    "line {i}: the quick brown fox jumps over the lazy dog {}\n",
                    i * i
                )
                .into_bytes()
            })
            .collect();
        let got = inflate(&dynamic, expected.len()).unwrap();
        assert_eq!(got, expected);
        assert_eq!(crc32(&got), 914_363_029);
    }
}
