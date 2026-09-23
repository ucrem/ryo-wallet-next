use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// One RYO contains exactly one billion atomic units.
const SCALE: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AtomicAmount(u64);

/// IPC representation: always decimal digits, never a JavaScript number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AtomicAmountDto {
    pub atomic: String,
}

impl From<AtomicAmount> for AtomicAmountDto {
    fn from(amount: AtomicAmount) -> Self {
        Self {
            atomic: amount.to_atomic_digits(),
        }
    }
}

impl TryFrom<AtomicAmountDto> for AtomicAmount {
    type Error = AmountError;

    fn try_from(value: AtomicAmountDto) -> Result<Self, Self::Error> {
        Self::parse_atomic_digits(&value.atomic)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    #[error("amount must be a plain non-negative decimal with at most nine fractional digits")]
    InvalidFormat,
    #[error("amount exceeds the supported atomic range")]
    Overflow,
}

impl AtomicAmount {
    pub const ZERO: Self = Self(0);

    pub const fn from_atomic(value: u64) -> Self {
        Self(value)
    }

    pub const fn atomic(self) -> u64 {
        self.0
    }

    /// Parse user-facing RYO without floating point or silent rounding.
    pub fn parse_ryo(input: &str) -> Result<Self, AmountError> {
        let (whole, fraction) = input.split_once('.').unwrap_or((input, ""));
        if whole.is_empty()
            || (input.contains('.') && fraction.is_empty())
            || fraction.len() > 9
            || !whole.bytes().all(|c| c.is_ascii_digit())
            || !fraction.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(AmountError::InvalidFormat);
        }
        let whole = whole.parse::<u64>().map_err(|_| AmountError::Overflow)?;
        let fraction = if fraction.is_empty() {
            0
        } else {
            fraction.parse::<u64>().map_err(|_| AmountError::Overflow)?
                * 10_u64.pow((9 - fraction.len()) as u32)
        };
        let atomic = whole
            .checked_mul(SCALE)
            .and_then(|v| v.checked_add(fraction))
            .ok_or(AmountError::Overflow)?;
        Ok(Self(atomic))
    }

    /// Parse an IPC amount. JavaScript must never convert this through `number`.
    pub fn parse_atomic_digits(input: &str) -> Result<Self, AmountError> {
        if input.is_empty() || !input.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AmountError::InvalidFormat);
        }
        input
            .parse::<u64>()
            .map(Self)
            .map_err(|_| AmountError::Overflow)
    }

    pub fn checked_add(self, other: Self) -> Result<Self, AmountError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(AmountError::Overflow)
    }

    pub fn to_ryo(self) -> String {
        let whole = self.0 / SCALE;
        let fraction = self.0 % SCALE;
        if fraction == 0 {
            return whole.to_string();
        }
        let mut value = format!("{whole}.{fraction:09}");
        while value.ends_with('0') {
            value.pop();
        }
        value
    }

    pub fn to_atomic_digits(self) -> String {
        self.0.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_round_trip_and_boundaries() {
        let cases = [
            ("0", 0),
            ("0.000000001", 1),
            ("12.34", 12_340_000_000),
            ("18446744073.709551615", u64::MAX),
        ];
        for (input, atomic) in cases {
            let amount = AtomicAmount::parse_ryo(input).unwrap();
            assert_eq!(amount.atomic(), atomic);
            assert_eq!(AtomicAmount::parse_ryo(&amount.to_ryo()).unwrap(), amount);
        }
        for input in [
            "",
            ".1",
            "1.",
            " 1",
            "+1",
            "-1",
            "1e2",
            "1.0000000001",
            "1,2",
        ] {
            assert_eq!(
                AtomicAmount::parse_ryo(input),
                Err(AmountError::InvalidFormat)
            );
        }
        assert_eq!(
            AtomicAmount::parse_ryo("18446744073.709551616"),
            Err(AmountError::Overflow)
        );
        assert_eq!(
            AtomicAmount::from_atomic(u64::MAX).checked_add(AtomicAmount::from_atomic(1)),
            Err(AmountError::Overflow)
        );
    }

    #[test]
    fn ipc_digits_do_not_accept_javascript_number_syntax() {
        assert_eq!(
            AtomicAmount::parse_atomic_digits("9007199254740993")
                .unwrap()
                .atomic(),
            9_007_199_254_740_993
        );
        for input in ["", "-1", "1.0", "1e3", " 1"] {
            assert_eq!(
                AtomicAmount::parse_atomic_digits(input),
                Err(AmountError::InvalidFormat)
            );
        }
    }
}
