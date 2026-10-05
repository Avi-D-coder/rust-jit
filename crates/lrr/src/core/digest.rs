//! Explicit domain-separated encoding. No Debug/serde/memory-layout hashing.
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, Visitor},
};
use std::{collections::BTreeMap, fmt};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Digest(pub [u8; 32]);
impl Digest {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut encoder = StableEncoder::new("lrr/v1/source-bytes");
        encoder.bytes(bytes);
        encoder.finish()
    }
    pub fn hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(HEX[usize::from(byte >> 4)] as char);
            output.push(HEX[usize::from(byte & 15)] as char);
        }
        output
    }
}
impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}
impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({self})")
    }
}
impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}
impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DigestVisitor;
        impl Visitor<'_> for DigestVisitor {
            type Value = Digest;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("exactly 64 lowercase hexadecimal characters")
            }
            fn visit_str<E: Error>(self, value: &str) -> Result<Digest, E> {
                if value.len() != 64 {
                    return Err(E::custom("digest length must be 64"));
                }
                let mut bytes = [0; 32];
                fn nibble<E: Error>(byte: u8) -> Result<u8, E> {
                    match byte {
                        b'0'..=b'9' => Ok(byte - b'0'),
                        b'a'..=b'f' => Ok(byte - b'a' + 10),
                        _ => Err(E::custom("digest must be lowercase hexadecimal")),
                    }
                }
                for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
                    bytes[i] = (nibble::<E>(pair[0])? << 4) | nibble::<E>(pair[1])?;
                }
                Ok(Digest(bytes))
            }
        }
        deserializer.deserialize_str(DigestVisitor)
    }
}

/// Control-plane hashing only; updates a hash state without allocating guest memory.
pub struct StableEncoder(blake3::Hasher);
impl StableEncoder {
    pub fn new(domain: &'static str) -> Self {
        let mut encoder = Self(blake3::Hasher::new());
        encoder.string(domain);
        encoder
    }
    pub fn u8(&mut self, value: u8) {
        self.0.update(&[value]);
    }
    pub fn u16(&mut self, value: u16) {
        self.0.update(&value.to_le_bytes());
    }
    pub fn u32(&mut self, value: u32) {
        self.0.update(&value.to_le_bytes());
    }
    pub fn u64(&mut self, value: u64) {
        self.0.update(&value.to_le_bytes());
    }
    pub fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        self.0.update(bytes);
    }
    pub fn string(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }
    pub fn digest(&mut self, digest: Digest) {
        self.0.update(&digest.0);
    }
    pub fn option<T>(&mut self, value: &Option<T>, encode: impl FnOnce(&mut Self, &T)) {
        match value {
            None => self.u32(0),
            Some(value) => {
                self.u32(1);
                encode(self, value);
            }
        }
    }
    pub fn sequence<T>(&mut self, values: &[T], mut encode: impl FnMut(&mut Self, &T)) {
        self.u64(values.len() as u64);
        for value in values {
            encode(self, value);
        }
    }
    pub fn map<K: Ord, V>(
        &mut self,
        values: &BTreeMap<K, V>,
        mut encode: impl FnMut(&mut Self, &K, &V),
    ) {
        self.u64(values.len() as u64);
        for (key, value) in values {
            encode(self, key, value);
        }
    }
    pub fn record<T: StableEncode>(&mut self, value: &T) {
        self.string(T::DOMAIN);
        value.encode_fields(self);
    }
    pub fn finish(self) -> Digest {
        Digest(*self.0.finalize().as_bytes())
    }
}
pub trait StableEncode {
    const DOMAIN: &'static str;
    fn encode_fields(&self, encoder: &mut StableEncoder);
    fn fingerprint(&self) -> Digest {
        let mut encoder = StableEncoder::new(Self::DOMAIN);
        self.encode_fields(&mut encoder);
        encoder.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digest_json_is_strict_lowercase_hex() {
        let digest = Digest::of_bytes(b"hello");
        let json = serde_json::to_string(&digest).unwrap();
        assert_eq!(json.len(), 66);
        assert_eq!(serde_json::from_str::<Digest>(&json).unwrap(), digest);
        assert!(serde_json::from_str::<Digest>(&json.to_uppercase()).is_err());
        assert!(serde_json::from_str::<Digest>("\"ab\"").is_err());
        assert!(serde_json::from_str::<Digest>(&format!("\"{}\"", "z".repeat(64))).is_err());
    }
    #[test]
    fn stable_encoding_has_explicit_widths_and_lengths() {
        let mut encoder = StableEncoder::new("lrr/v1/test");
        encoder.u8(3);
        encoder.u16(0x0102);
        encoder.u32(0x03040506);
        encoder.u64(0x0708090a0b0c0d0e);
        encoder.string("é");
        let mut bytes = Vec::new();
        bytes.extend(11_u64.to_le_bytes());
        bytes.extend(b"lrr/v1/test");
        bytes.push(3);
        bytes.extend(0x0102_u16.to_le_bytes());
        bytes.extend(0x03040506_u32.to_le_bytes());
        bytes.extend(0x0708090a0b0c0d0e_u64.to_le_bytes());
        bytes.extend(2_u64.to_le_bytes());
        bytes.extend("é".as_bytes());
        assert_eq!(encoder.finish(), Digest(*blake3::hash(&bytes).as_bytes()));
    }
    #[test]
    fn maps_are_independent_of_insertion_order() {
        let a = BTreeMap::from([("b", 2_u32), ("a", 1)]);
        let b = BTreeMap::from([("a", 1_u32), ("b", 2)]);
        let fingerprint = |map: &BTreeMap<&str, u32>| {
            let mut e = StableEncoder::new("lrr/v1/map-test");
            e.map(map, |e, k, v| {
                e.string(k);
                e.u32(*v);
            });
            e.finish()
        };
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }
    #[test]
    fn fields_order_versions_and_domains_change_hashes() {
        let encode = |domain, a, b| {
            let mut e = StableEncoder::new(domain);
            e.string(a);
            e.string(b);
            e.finish()
        };
        assert_ne!(
            encode("lrr/v1/test", "ab", "c"),
            encode("lrr/v1/test", "a", "bc")
        );
        assert_ne!(
            encode("lrr/v1/test", "a", "b"),
            encode("lrr/v1/test", "b", "a")
        );
        assert_ne!(
            encode("lrr/v1/test", "a", "b"),
            encode("lrr/v1/other", "a", "b")
        );
        assert_ne!(
            encode("lrr/v1/test", "a", "b"),
            encode("lrr/v2/test", "a", "b")
        );
    }
}
