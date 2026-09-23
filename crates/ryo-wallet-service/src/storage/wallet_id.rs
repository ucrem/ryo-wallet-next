use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Opaque, fixed-format app identifier; it is never a user-visible filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WalletId(String);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("wallet identifier must be exactly 32 lowercase hexadecimal characters")]
pub struct WalletIdError;

impl WalletId {
    pub fn parse(value: impl Into<String>) -> Result<Self, WalletIdError> {
        let value = value.into();
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(WalletIdError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for WalletId {
    type Err = WalletIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for WalletId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fixed_lowercase_hex_ids_are_accepted() {
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(id.as_str(), "0123456789abcdef0123456789abcdef");
        for value in [
            "",
            "../wallet",
            "A123456789abcdef0123456789abcdef",
            "0123456789abcdef",
        ] {
            assert_eq!(WalletId::parse(value), Err(WalletIdError));
        }
    }
}
