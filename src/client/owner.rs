//! The host owner's nickname key. Login strips it, so only the visible name
//! reaches the world, packets, records and logs.
use super::simulation::MAX_NICKNAME;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A host credential serialized as a string, but never revealed by `Debug`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OwnerKey(String);

impl fmt::Debug for OwnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnerKey([redacted])")
    }
}

impl From<String> for OwnerKey {
    fn from(key: String) -> Self {
        Self(key)
    }
}

impl OwnerKey {
    /// The `!` marker and key must fit one nickname. Keys use the protocol's
    /// Latin-1 bytes: visible ASCII or Latin-1 symbols and letters, without
    /// spaces, controls or the invisible soft hyphen.
    pub(super) fn validate(&self) -> Result<(), String> {
        let printable =
            |c: char| matches!(c, '\u{21}'..='\u{7e}' | '\u{a1}'..='\u{ff}') && c != '\u{ad}';
        if self.0.is_empty()
            || self.0.chars().count() >= MAX_NICKNAME
            || !self.0.chars().all(printable)
        {
            return Err(
                "ownerKey must contain 1 to 23 printable Latin-1 characters without whitespace"
                    .into(),
            );
        }
        Ok(())
    }

    /// Short keys are easy to guess; the host warns about them.
    pub(super) fn short(&self) -> bool {
        self.0.chars().count() < 6
    }

    /// Remove a final `!` plus the exact key, then the ASCII spaces directly
    /// before it. Matches the protocol's Latin-1 bytes, not UTF-8. A name
    /// without the marker stays exactly as typed.
    pub(super) fn strip(&self, nickname: &mut Vec<u8>) -> bool {
        let marker: Vec<_> = std::iter::once(b'!')
            .chain(self.0.chars().map(|c| c as u8))
            .collect();
        if !nickname.ends_with(&marker) {
            return false;
        }
        nickname.truncate(nickname.len() - marker.len());
        while nickname.last() == Some(&b' ') {
            nickname.pop();
        }
        true
    }
}
