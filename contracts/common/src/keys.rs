extern crate alloc;

use alloc::string::String as StdString;
use alloc::vec::Vec as StdVec;

#[derive(Debug, Clone)]
pub struct DataKey {
    pub id: StdString,
    pub key: StdVec<u8>,
    pub created: u64,
    pub expires: Option<u64>,
}

#[derive(Default)]
pub struct KeyManager {
    pub master: StdVec<u8>,
    // Lightweight in-memory map kept for host-side usage; not used in-contract.
    pub data_keys: alloc::collections::BTreeMap<StdString, DataKey>,
}

impl KeyManager {
    pub fn new(master: StdVec<u8>) -> Self {
        Self {
            master,
            data_keys: alloc::collections::BTreeMap::new(),
        }
    }

    pub fn create_data_key(&mut self, id: &str, key: StdVec<u8>, ttl: Option<u64>) {
        let now = 0u64; // timestamping omitted for deterministic host-side tests
        self.data_keys.insert(
            id.into(),
            DataKey {
                id: id.into(),
                key,
                created: now,
                expires: ttl.and_then(|t| now.checked_add(t)),
            },
        );
    }

    pub fn rotate_master(&mut self, new_master: StdVec<u8>) {
        self.master = new_master;
    }

    pub fn get_key(&self, id: &str) -> Option<&DataKey> {
        self.data_keys.get(id)
    }

    /// Encrypt plaintext using a specific data key (if present) or the manager master key.
    /// This uses a simple XOR stream with the key and hex-encodes the result.
    pub fn encrypt(&self, key_id: Option<&str>, plaintext: &str) -> StdString {
        let key = key_id
            .and_then(|id| self.get_key(id).map(|dk| dk.key.as_slice()))
            .unwrap_or(self.master.as_slice());
        xor_and_hex_encode(key, plaintext.as_bytes())
    }

    /// Decrypt the hex-encoded ciphertext produced by `encrypt`.
    pub fn decrypt(&self, key_id: Option<&str>, ciphertext_hex: &str) -> Option<StdString> {
        let key = key_id
            .and_then(|id| self.get_key(id).map(|dk| dk.key.as_slice()))
            .unwrap_or(self.master.as_slice());
        hex_decode_and_xor(key, ciphertext_hex)
    }
}

/// Helper: XOR plaintext bytes with key (repeating) then hex-encode the result.
fn xor_and_hex_encode(key: &[u8], plaintext: &[u8]) -> StdString {
    let mut out = StdVec::with_capacity(plaintext.len());
    if key.is_empty() {
        // no-op encoding
        out.extend_from_slice(plaintext);
    } else {
        for (i, b) in plaintext.iter().enumerate() {
            out.push(b ^ key[i % key.len()]);
        }
    }
    // hex encode
    let mut s = StdString::with_capacity(out.len() * 2);
    for byte in out {
        let hi = nibble_to_hex((byte >> 4) & 0xF);
        let lo = nibble_to_hex(byte & 0xF);
        s.push(hi);
        s.push(lo);
    }
    s
}

fn nibble_to_hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '?',
    }
}

fn hex_char_val(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some((c as u8) - b'0'),
        'a'..='f' => Some((c as u8) - b'a' + 10),
        'A'..='F' => Some((c as u8) - b'A' + 10),
        _ => None,
    }
}

fn hex_decode_and_xor(key: &[u8], hexstr: &str) -> Option<StdString> {
    let chars: StdVec<char> = hexstr.chars().collect();
    if chars.len() % 2 != 0 {
        return None;
    }
    let mut bytes = StdVec::with_capacity(chars.len() / 2);
    let mut i = 0usize;
    while i < chars.len() {
        let hi = hex_char_val(chars[i])?;
        let lo = hex_char_val(chars[i + 1])?;
        bytes.push((hi << 4) | lo);
        i += 2;
    }
    // XOR with key
    let mut out = StdVec::with_capacity(bytes.len());
    if key.is_empty() {
        out.extend_from_slice(&bytes);
    } else {
        for (i, b) in bytes.iter().enumerate() {
            out.push(b ^ key[i % key.len()]);
        }
    }
    match StdString::from_utf8(out) {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}

/// Decode a hex string into raw bytes. Returns `None` on invalid input.
pub fn hex_to_bytes(hexstr: &str) -> Option<StdVec<u8>> {
    let chars: StdVec<char> = hexstr.chars().collect();
    if chars.len() % 2 != 0 {
        return None;
    }
    let mut bytes = StdVec::with_capacity(chars.len() / 2);
    let mut i = 0usize;
    while i < chars.len() {
        let hi = hex_char_val(chars[i])?;
        let lo = hex_char_val(chars[i + 1])?;
        bytes.push((hi << 4) | lo);
        i += 2;
    }
    Some(bytes)
}

/// Encode raw bytes into a lowercase hex string.
pub fn bytes_to_hex(bytes: &[u8]) -> StdString {
    let mut s = StdString::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(nibble_to_hex((b >> 4) & 0xF));
        s.push(nibble_to_hex(b & 0xF));
    }
    s
}
