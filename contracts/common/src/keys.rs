#![allow(dead_code, clippy::incompatible_msrv)]

extern crate alloc;

use alloc::string::String as StdString;
use alloc::vec::Vec as StdVec;

// ── Shared helpers ───────────────────────────────────────────────────────────

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

fn xor_and_hex_encode(key: &[u8], plaintext: &[u8]) -> StdString {
    let mut out = StdVec::with_capacity(plaintext.len());
    if key.is_empty() {
        out.extend_from_slice(plaintext);
    } else {
        for (i, b) in plaintext.iter().enumerate() {
            out.push(b ^ key[i % key.len()]);
        }
    }

    let mut s = StdString::with_capacity(out.len() * 2);
    for byte in out {
        s.push(nibble_to_hex((byte >> 4) & 0xF));
        s.push(nibble_to_hex(byte & 0xF));
    }
    s
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

    let mut out = StdVec::with_capacity(bytes.len());
    if key.is_empty() {
        out.extend_from_slice(&bytes);
    } else {
        for (i, b) in bytes.iter().enumerate() {
            out.push(b ^ key[i % key.len()]);
        }
    }

    StdString::from_utf8(out).ok()
}

// ── Soroban (no_std) implementation ─────────────────────────────────────────

#[cfg(not(any(test, feature = "std")))]
mod soroban_impl {
    use super::{hex_decode_and_xor, xor_and_hex_encode, StdVec};
    use soroban_sdk::{Bytes, Env, String};
    extern crate alloc;
    use alloc::string::ToString;

    #[derive(Clone)]
    pub struct KeyManager {
        pub master: Bytes,
    }

    impl KeyManager {
        pub fn new(master: Bytes) -> Self {
            Self { master }
        }

        pub fn encrypt(&self, env: &Env, plaintext: String) -> String {
            let key = bytes_from_soroban(&self.master);
            let cipher = xor_and_hex_encode(&key, plaintext.to_string().as_bytes());
            String::from_str(env, &cipher)
        }

        pub fn decrypt(&self, env: &Env, ciphertext: String) -> Option<String> {
            let key = bytes_from_soroban(&self.master);
            let plain = hex_decode_and_xor(&key, &ciphertext.to_string())?;
            Some(String::from_str(env, &plain))
        }
    }

    pub fn hex_to_bytes(env: &Env, hexstr: String) -> Option<Bytes> {
        let raw = hex_to_vec(&hexstr.to_string())?;
        let mut out = Bytes::new(env);
        for b in raw {
            out.push_back(b);
        }
        Some(out)
    }

    fn hex_to_vec(hexstr: &str) -> Option<StdVec<u8>> {
        let chars: StdVec<char> = hexstr.chars().collect();
        if chars.len() % 2 != 0 {
            return None;
        }
        let mut bytes = StdVec::with_capacity(chars.len() / 2);
        let mut i = 0usize;
        while i < chars.len() {
            let hi = super::hex_char_val(chars[i])?;
            let lo = super::hex_char_val(chars[i + 1])?;
            bytes.push((hi << 4) | lo);
            i += 2;
        }
        Some(bytes)
    }

    fn bytes_from_soroban(bytes: &Bytes) -> StdVec<u8> {
        let mut out = StdVec::with_capacity(bytes.len() as usize);
        let mut i = 0u32;
        while i < bytes.len() {
            out.push(bytes.get(i).unwrap_or(0));
            i += 1;
        }
        out
    }
}

// ── Std/test implementation ─────────────────────────────────────────────────

#[cfg(any(test, feature = "std"))]
mod std_impl {
    extern crate alloc;
    use self::alloc::collections::BTreeMap;
    use super::{hex_decode_and_xor, xor_and_hex_encode, StdString, StdVec};

    #[derive(Debug, Clone, Default)]
    pub struct AuditEntry {
        pub actor: StdString,
        pub action: StdString,
        pub target: StdString,
        pub timestamp: u64,
    }

    #[derive(Default)]
    pub struct AuditLog {
        pub entries: StdVec<AuditEntry>,
    }

    impl AuditLog {
        pub fn record(&mut self, actor: &str, action: &str, target: &str, now: u64) {
            self.entries.push(AuditEntry {
                actor: StdString::from(actor),
                action: StdString::from(action),
                target: StdString::from(target),
                timestamp: now,
            });
        }

        pub fn query(&self) -> &[AuditEntry] {
            &self.entries
        }
    }

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
        pub data_keys: BTreeMap<StdString, DataKey>,
        pub old_master: Option<StdVec<u8>>,
    }

    impl KeyManager {
        pub fn new(master: StdVec<u8>) -> Self {
            Self {
                master,
                data_keys: BTreeMap::new(),
                old_master: None,
            }
        }

        pub fn create_data_key(&mut self, id: &str, key: StdVec<u8>, ttl: Option<u64>, now: u64) {
            self.data_keys.insert(
                StdString::from(id),
                DataKey {
                    id: StdString::from(id),
                    key,
                    created: now,
                    expires: ttl.and_then(|t| now.checked_add(t)),
                },
            );
        }

        pub fn rotate_master(&mut self, new_master: StdVec<u8>) {
            self.master = new_master;
        }

        pub fn rotate_master_secure(
            &mut self,
            new_master: StdVec<u8>,
            audit: &mut AuditLog,
            actor: &str,
            now: u64,
        ) {
            self.old_master = Some(self.master.clone());

            for dk in self.data_keys.values_mut() {
                for (i, b) in dk.key.iter_mut().enumerate() {
                    *b ^= self.master.get(i % self.master.len()).unwrap_or(&0);
                }
                for (i, b) in dk.key.iter_mut().enumerate() {
                    *b ^= new_master.get(i % new_master.len()).unwrap_or(&0);
                }
            }

            for b in &mut self.master {
                *b = 0;
            }
            self.master = new_master;
            audit.record(actor, "rotate_master_secure", "master_key", now);
        }

        pub fn get_key(&self, id: &str) -> Option<&DataKey> {
            self.data_keys.get(id)
        }

        pub fn encrypt(&self, key_id: Option<&str>, plaintext: &str) -> StdString {
            let key = key_id
                .and_then(|id| self.get_key(id).map(|dk| dk.key.as_slice()))
                .unwrap_or(self.master.as_slice());
            xor_and_hex_encode(key, plaintext.as_bytes())
        }

        pub fn decrypt(&self, key_id: Option<&str>, ciphertext_hex: &str) -> Option<StdString> {
            let key = key_id
                .and_then(|id| self.get_key(id).map(|dk| dk.key.as_slice()))
                .unwrap_or(self.master.as_slice());
            hex_decode_and_xor(key, ciphertext_hex)
        }
    }

    pub fn hex_to_bytes(hexstr: &str) -> Option<StdVec<u8>> {
        let chars: StdVec<char> = hexstr.chars().collect();
        if chars.len() % 2 != 0 {
            return None;
        }

        let mut bytes = StdVec::with_capacity(chars.len() / 2);
        let mut i = 0usize;
        while i < chars.len() {
            let hi = super::hex_char_val(chars[i])?;
            let lo = super::hex_char_val(chars[i + 1])?;
            bytes.push((hi << 4) | lo);
            i += 2;
        }
        Some(bytes)
    }

    pub fn bytes_to_hex(bytes: &[u8]) -> StdString {
        let mut s = StdString::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(super::nibble_to_hex((b >> 4) & 0xF));
            s.push(super::nibble_to_hex(b & 0xF));
        }
        s
    }
}

#[cfg(not(any(test, feature = "std")))]
pub use soroban_impl::{hex_to_bytes, KeyManager};

#[cfg(any(test, feature = "std"))]
pub use std_impl::{bytes_to_hex, hex_to_bytes, AuditEntry, AuditLog, DataKey, KeyManager};
