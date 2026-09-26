//! Read-only, account-zero transaction snapshot from Ryo's `get_transfers` RPC.
//! The group in the response is authoritative; upstream `type` is not exposed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::transport::RpcError;

pub const ACTIVITY_LIMIT: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ActivityDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ActivityStatus {
    Confirmed,
    Pending,
    Pool,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ActivityEntry {
    pub txid: String,
    pub direction: ActivityDirection,
    pub status: ActivityStatus,
    pub amount_atomic: String,
    pub fee_atomic: Option<String>,
    pub height: Option<String>,
    pub timestamp: Option<String>,
    pub payment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ActivitySnapshot {
    pub transactions: Vec<ActivityEntry>,
    pub truncated: bool,
    // Pool refresh needs a reachable daemon; other history remains available offline.
    pub pool_unavailable: bool,
}

#[derive(Deserialize)]
pub(crate) struct RawTransfers {
    #[serde(default, rename = "in")]
    incoming: Vec<RawTransfer>,
    #[serde(default, rename = "out")]
    outgoing: Vec<RawTransfer>,
    #[serde(default)]
    pending: Vec<RawTransfer>,
    #[serde(default)]
    failed: Vec<RawTransfer>,
    #[serde(default)]
    pool: Vec<RawTransfer>,
}

#[derive(Deserialize)]
struct RawTransfer {
    txid: String,
    payment_id: String,
    height: u64,
    timestamp: u64,
    amount: u64,
    fee: u64,
}

struct OrderedEntry {
    entry: ActivityEntry,
    timestamp: u64,
    height: u64,
}

impl RawTransfers {
    pub(crate) fn merge_pool(&mut self, pool: Self) -> Result<(), RpcError> {
        if !pool.incoming.is_empty()
            || !pool.outgoing.is_empty()
            || !pool.pending.is_empty()
            || !pool.failed.is_empty()
        {
            return Err(RpcError::InvalidResponse);
        }
        self.pool = pool.pool;
        Ok(())
    }

    pub(crate) fn normalize(self) -> Result<ActivitySnapshot, RpcError> {
        let groups = [
            (
                self.incoming,
                ActivityDirection::Incoming,
                ActivityStatus::Confirmed,
            ),
            (
                self.outgoing,
                ActivityDirection::Outgoing,
                ActivityStatus::Confirmed,
            ),
            (
                self.pending,
                ActivityDirection::Outgoing,
                ActivityStatus::Pending,
            ),
            (
                self.failed,
                ActivityDirection::Outgoing,
                ActivityStatus::Failed,
            ),
            (self.pool, ActivityDirection::Incoming, ActivityStatus::Pool),
        ];
        let count = groups
            .iter()
            .try_fold(0_usize, |total, (items, _, _)| {
                total.checked_add(items.len())
            })
            .ok_or(RpcError::InvalidResponse)?;
        let mut entries = Vec::with_capacity(count);
        for (items, direction, status) in groups {
            for raw in items {
                entries.push(raw.normalize(direction, status)?);
            }
        }
        entries.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| right.height.cmp(&left.height))
                .then_with(|| left.entry.txid.cmp(&right.entry.txid))
                .then_with(|| left.entry.status.cmp(&right.entry.status))
                .then_with(|| left.entry.direction.cmp(&right.entry.direction))
                .then_with(|| left.entry.amount_atomic.cmp(&right.entry.amount_atomic))
        });
        let truncated = entries.len() > ACTIVITY_LIMIT;
        entries.truncate(ACTIVITY_LIMIT);
        Ok(ActivitySnapshot {
            transactions: entries.into_iter().map(|item| item.entry).collect(),
            truncated,
            pool_unavailable: false,
        })
    }
}

impl RawTransfer {
    fn normalize(
        self,
        direction: ActivityDirection,
        status: ActivityStatus,
    ) -> Result<OrderedEntry, RpcError> {
        if !valid_hex(&self.txid, &[64]) {
            return Err(RpcError::InvalidResponse);
        }
        let payment_id = if self.payment_id.is_empty() {
            None
        } else if valid_hex(&self.payment_id, &[16, 64]) {
            (!self.payment_id.bytes().all(|byte| byte == b'0'))
                .then(|| self.payment_id.to_ascii_lowercase())
        } else {
            return Err(RpcError::InvalidResponse);
        };
        let entry = ActivityEntry {
            txid: self.txid.to_ascii_lowercase(),
            direction,
            status,
            amount_atomic: self.amount.to_string(),
            fee_atomic: (direction == ActivityDirection::Outgoing).then(|| self.fee.to_string()),
            height: (self.height != 0).then(|| self.height.to_string()),
            timestamp: (self.timestamp != 0).then(|| self.timestamp.to_string()),
            payment_id,
        };
        Ok(OrderedEntry {
            entry,
            timestamp: self.timestamp,
            height: self.height,
        })
    }
}

fn valid_hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn transfer(txid: &str, amount: u64, fee: u64, height: u64, timestamp: u64) -> Value {
        json!({
            "txid": txid, "payment_id": "0000000000000000", "amount": amount,
            "fee": fee, "height": height, "timestamp": timestamp,
            "type": "untrusted", "tx_key": "must never cross IPC",
        })
    }

    fn groups() -> Value {
        json!({ "in": [], "out": [], "pending": [], "failed": [], "pool": [] })
    }

    fn normalize(value: Value) -> Result<ActivitySnapshot, RpcError> {
        serde_json::from_value::<RawTransfers>(value)
            .map_err(|_| RpcError::InvalidResponse)?
            .normalize()
    }

    #[test]
    fn normalizes_each_upstream_group_without_trusting_type_or_exposing_raw_fields() {
        let mut raw = groups();
        let id = "a".repeat(64);
        raw["in"] = json!([transfer(&id, 125_000_000_000, 55, 10, 100)]);
        raw["out"] = json!([transfer(&"b".repeat(64), 18_500_000_000, 9, 11, 110)]);
        raw["pending"] = json!([transfer(&"c".repeat(64), 5, 2, 0, 120)]);
        raw["failed"] = json!([transfer(&"d".repeat(64), 6, 3, 0, 130)]);
        raw["pool"] = json!([transfer(&"e".repeat(64), 7, 4, 0, 140)]);
        let snapshot = normalize(raw).unwrap();
        assert!(!snapshot.pool_unavailable);
        let entries = &snapshot.transactions;
        assert_eq!(
            entries.iter().map(|item| item.status).collect::<Vec<_>>(),
            [
                ActivityStatus::Pool,
                ActivityStatus::Failed,
                ActivityStatus::Pending,
                ActivityStatus::Confirmed,
                ActivityStatus::Confirmed,
            ]
        );
        assert_eq!(entries[0].direction, ActivityDirection::Incoming);
        assert_eq!(entries[1].direction, ActivityDirection::Outgoing);
        assert_eq!(entries[3].amount_atomic, "18500000000");
        assert_eq!(entries[3].fee_atomic.as_deref(), Some("9"));
        assert_eq!(entries[4].fee_atomic, None);
        assert_eq!(entries[2].height, None);
        assert_eq!(entries[4].height.as_deref(), Some("10"));
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains("untrusted"));
        assert!(!encoded.contains("tx_key"));
    }

    #[test]
    fn rejects_malformed_hash_payment_id_and_amount_without_panicking() {
        for bad_id in ["", "x", "z".repeat(64).as_str(), "a\n".repeat(32).as_str()] {
            let mut raw = groups();
            raw["in"] = json!([transfer(bad_id, 1, 0, 1, 1)]);
            assert_eq!(normalize(raw), Err(RpcError::InvalidResponse));
        }
        let mut raw = groups();
        raw["out"] = json!([transfer(&"a".repeat(64), 1, 0, 1, 1)]);
        raw["out"][0]["payment_id"] = json!("bad\nvalue");
        assert_eq!(normalize(raw), Err(RpcError::InvalidResponse));
        let mut raw = groups();
        raw["in"] = json!([transfer(&"a".repeat(64), 1, 0, 1, 1)]);
        raw["in"][0]["amount"] = json!("not an integer");
        assert_eq!(normalize(raw), Err(RpcError::InvalidResponse));
    }

    #[test]
    fn sorts_deterministically_and_marks_a_bounded_snapshot() {
        let mut raw = groups();
        raw["in"] = Value::Array(
            (0..251)
                .rev()
                .map(|index| transfer(&format!("{index:064x}"), index, 0, 20, 100))
                .collect(),
        );
        let snapshot = normalize(raw).unwrap();
        assert!(snapshot.truncated);
        assert_eq!(snapshot.transactions.len(), ACTIVITY_LIMIT);
        assert_eq!(snapshot.transactions[0].txid, format!("{:064x}", 0));
        assert_eq!(
            snapshot.transactions.last().unwrap().txid,
            format!("{:064x}", 249)
        );
    }

    #[test]
    fn keeps_large_values_as_exact_decimal_strings_and_validates_payment_id() {
        let mut raw = groups();
        raw["out"] = json!([transfer(
            &"f".repeat(64),
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX
        )]);
        raw["out"][0]["payment_id"] = json!("ABCDEF1234567890");
        let entry = &normalize(raw).unwrap().transactions[0];
        assert_eq!(entry.amount_atomic, u64::MAX.to_string());
        assert_eq!(
            entry.fee_atomic.as_deref(),
            Some(u64::MAX.to_string().as_str())
        );
        assert_eq!(entry.height.as_deref(), Some(u64::MAX.to_string().as_str()));
        assert_eq!(entry.payment_id.as_deref(), Some("abcdef1234567890"));
    }

    #[test]
    fn merges_a_separate_pool_response_and_rejects_unexpected_groups() {
        let mut base: RawTransfers = serde_json::from_value(groups()).unwrap();
        let mut pool = groups();
        pool["pool"] = json!([transfer(&"a".repeat(64), 1, 0, 0, 1)]);
        base.merge_pool(serde_json::from_value(pool).unwrap())
            .unwrap();
        assert_eq!(
            base.normalize().unwrap().transactions[0].status,
            ActivityStatus::Pool
        );

        let mut base: RawTransfers = serde_json::from_value(groups()).unwrap();
        let mut bad = groups();
        bad["out"] = json!([transfer(&"b".repeat(64), 1, 0, 0, 1)]);
        assert_eq!(
            base.merge_pool(serde_json::from_value(bad).unwrap()),
            Err(RpcError::InvalidResponse)
        );
    }
}
