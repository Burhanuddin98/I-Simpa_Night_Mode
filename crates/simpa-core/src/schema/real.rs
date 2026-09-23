//! [`F64`], the float every project value is stored in, and [`Vec3`].

use std::borrow::Cow;
use std::fmt;
use std::hash::{Hash, Hasher};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Bits of the one NaN spelled `"NaN"` in a project file: the positive quiet NaN.
pub(crate) const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

/// An `f64` that is persisted bit-exactly and compares by bits.
///
/// Equality and hashing use the bit pattern, so `NaN == NaN` when the bits match and
/// `0.0 != -0.0`. That makes "has anything changed" and "did this round-trip exactly" the same
/// question as `==`. Arithmetic goes through [`F64::get`].
///
/// In JSON a finite value is a number and a non-finite value is a string (see the module docs of
/// `schema` for the exact spelling).
#[derive(Clone, Copy, Default)]
pub struct F64(f64);

impl F64 {
    pub const ZERO: F64 = F64(0.0);

    pub const fn new(value: f64) -> Self {
        F64(value)
    }

    pub const fn get(self) -> f64 {
        self.0
    }

    pub const fn to_bits(self) -> u64 {
        self.0.to_bits()
    }

    pub const fn from_bits(bits: u64) -> Self {
        F64(f64::from_bits(bits))
    }
}

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for F64 {}

impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl fmt::Debug for F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_nan() {
            write!(f, "NaN(0x{:016x})", self.0.to_bits())
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

impl fmt::Display for F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<f64> for F64 {
    fn from(value: f64) -> Self {
        F64(value)
    }
}

impl From<F64> for f64 {
    fn from(value: F64) -> Self {
        value.0
    }
}

/// The string spelling of a non-finite value.
fn encode_non_finite(value: f64) -> String {
    let bits = value.to_bits();
    if value == f64::INFINITY {
        "Infinity".to_string()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".to_string()
    } else if bits == CANONICAL_NAN_BITS {
        "NaN".to_string()
    } else {
        format!("NaN:0x{bits:016x}")
    }
}

/// Inverse of [`encode_non_finite`]. `"NaN:0x..."` must hold 16 hex digits of a NaN pattern.
fn decode_non_finite(text: &str) -> Option<f64> {
    match text {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::from_bits(CANONICAL_NAN_BITS)),
        _ => {
            let hex = text.strip_prefix("NaN:0x")?;
            if hex.len() != 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let value = f64::from_bits(u64::from_str_radix(hex, 16).ok()?);
            value.is_nan().then_some(value)
        }
    }
}

impl Serialize for F64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_finite() {
            serializer.serialize_f64(self.0)
        } else {
            serializer.serialize_str(&encode_non_finite(self.0))
        }
    }
}

struct F64Visitor;

impl Visitor<'_> for F64Visitor {
    type Value = F64;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "a number, or one of \"NaN\", \"Infinity\", \"-Infinity\", \"NaN:0x<16 hex digits>\"",
        )
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<F64, E> {
        Ok(F64(v))
    }

    // Integer literals: `as` rounds to nearest, ties to even, which is the correctly rounded
    // value of the decimal integer.
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<F64, E> {
        Ok(F64(v as f64))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<F64, E> {
        Ok(F64(v as f64))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<F64, E> {
        decode_non_finite(v)
            .map(F64)
            .ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &self))
    }
}

impl<'de> Deserialize<'de> for F64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(F64Visitor)
    }
}

impl JsonSchema for F64 {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "F64".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A float. Finite values are numbers; non-finite values are strings.",
            "oneOf": [
                { "type": "number" },
                {
                    "type": "string",
                    "pattern": "^(NaN|Infinity|-Infinity|NaN:0x[0-9a-fA-F]{16})$"
                }
            ]
        })
    }
}

/// A point or direction in world metres, Z up. In JSON: `[x, y, z]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vec3 {
    pub x: F64,
    pub y: F64,
    pub z: F64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 {
            x: F64(x),
            y: F64(y),
            z: F64(z),
        }
    }

    pub const fn to_array(self) -> [f64; 3] {
        [self.x.0, self.y.0, self.z.0]
    }

    /// Euclidean length.
    pub fn length(self) -> f64 {
        let [x, y, z] = self.to_array();
        (x * x + y * y + z * z).sqrt()
    }
}

impl From<[f64; 3]> for Vec3 {
    fn from([x, y, z]: [f64; 3]) -> Self {
        Vec3::new(x, y, z)
    }
}

impl Serialize for Vec3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y, self.z].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Vec3 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [x, y, z] = <[F64; 3]>::deserialize(deserializer)?;
        Ok(Vec3 { x, y, z })
    }
}

impl JsonSchema for Vec3 {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Vec3".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "[x, y, z] in metres, Z up",
            "type": "array",
            "items": generator.subschema_for::<F64>(),
            "minItems": 3,
            "maxItems": 3
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_is_bitwise() {
        assert_ne!(F64::new(0.0), F64::new(-0.0));
        assert_eq!(F64::new(f64::NAN), F64::new(f64::NAN));
        assert_ne!(
            F64::from_bits(CANONICAL_NAN_BITS),
            F64::from_bits(CANONICAL_NAN_BITS | 1)
        );
    }

    #[test]
    fn non_finite_spellings_invert() {
        for bits in [
            CANONICAL_NAN_BITS,
            CANONICAL_NAN_BITS | 1,
            0xfff8_0000_0000_0000,
            0x7ff0_0000_0000_0001,
            0xffff_ffff_ffff_ffff,
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
        ] {
            let text = encode_non_finite(f64::from_bits(bits));
            assert_eq!(
                decode_non_finite(&text).map(f64::to_bits),
                Some(bits),
                "{text}"
            );
        }
        assert_eq!(decode_non_finite("NaN:0x3ff0000000000000"), None);
        assert_eq!(decode_non_finite("NaN:0x7ff800000000000"), None);
        assert_eq!(decode_non_finite("nan"), None);
        assert_eq!(decode_non_finite("inf"), None);
    }
}
