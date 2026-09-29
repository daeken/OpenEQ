//! Client-side NPC service state. Catalog prices and transaction outcomes come
//! from EQEmu; local enablement never substitutes for a server result.
use openeq_net::{
    gameplay::{CoinLocation, CoinType, Currency},
    inventory::{InventoryItem, InventorySlot},
};
use std::{collections::BTreeMap, time::Instant};

pub const SERVICE_RANGE: f32 = 200.;
pub const BANKER_CLASS: u8 = 40;
pub const MERCHANT_CLASS: u8 = 41;

pub struct MerchantSession {
    pub id: u32,
    pub name: String,
    pub opened: bool,
    pub rate: f32,
    pub items: BTreeMap<u32, InventoryItem>,
    pub status: String,
}
impl MerchantSession {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            opened: false,
            rate: 0.,
            items: BTreeMap::new(),
            status: "Waiting for merchant…".into(),
        }
    }
}

pub struct BankSession {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub enum TransactionKind {
    Buy { slot: u32, quantity: u32 },
    Sell { slot: InventorySlot, quantity: u32 },
}
#[derive(Clone, Debug)]
pub struct PendingTransaction {
    pub merchant_id: u32,
    pub kind: TransactionKind,
    pub started: Instant,
    /// The worker dispatched this request; a delayed server result must still
    /// reconcile after death closes the merchant. This is not a success ACK.
    pub sent: bool,
}

#[derive(Default)]
pub struct CommerceState {
    pub merchant: Option<MerchantSession>,
    pub bank: Option<BankSession>,
    /// Kept after closing the merchant so a delayed acknowledgment still applies.
    pub pending: Option<PendingTransaction>,
    pub bank_money: Currency,
    pub shared_platinum: u32,
    pub cursor_money: Currency,
    /// The Storage2 deployment explicitly enables this EQEmu server rule.
    pub shared_coin_enabled: bool,
    pub coin_pending: bool,
    pub currency_ready: bool,
    /// Close replies have no merchant ID. Await the old reply before reopening.
    pub merchant_closing: bool,
}
impl CommerceState {
    pub fn close_services(&mut self) {
        self.merchant = None;
        self.bank = None;
        self.merchant_closing = false;
    }
    pub(crate) fn retire_queued_transactions(&mut self) {
        // The worker publishes sent callbacks before processing the event that
        // retires their epoch. Remaining unsent work can no longer be sent.
        self.coin_pending = false;
        if self.pending.as_ref().is_some_and(|pending| !pending.sent) {
            self.pending = None;
        }
    }
    pub fn pending_message(&self) -> Option<&'static str> {
        self.pending.as_ref().map(|pending| {
            if pending.started.elapsed().as_secs() >= 12 {
                "No transaction confirmation received. Reconnect to synchronize before trading again."
            } else { "Waiting for server confirmation…" }
        })
    }
}

pub fn total_copper(money: Currency) -> u64 {
    u64::from(money.platinum) * 1000
        + u64::from(money.gold) * 100
        + u64::from(money.silver) * 10
        + u64::from(money.copper)
}

pub fn debit(money: &mut Currency, amount: u32) -> bool {
    if total_copper(*money) < u64::from(amount) {
        return false;
    }
    let mut remaining = u64::from(amount);
    let mut coins = [money.copper, money.silver, money.gold, money.platinum];
    let units = [1u64, 10, 100, 1000];
    for i in 0..4 {
        let value = u64::from(coins[i]) * units[i];
        if value <= remaining {
            coins[i] = 0;
            remaining -= value;
        } else {
            let change = value - remaining;
            coins[i] = (change / units[i]) as u32;
            let mut lower = change % units[i];
            for j in (0..i).rev() {
                coins[j] = (lower / units[j]) as u32;
                lower %= units[j];
            }
            break;
        }
    }
    *money = Currency {
        copper: coins[0],
        silver: coins[1],
        gold: coins[2],
        platinum: coins[3],
    };
    true
}
fn denomination(money: Currency, coin: CoinType) -> u32 {
    match coin {
        CoinType::Copper => money.copper,
        CoinType::Silver => money.silver,
        CoinType::Gold => money.gold,
        CoinType::Platinum => money.platinum,
    }
}
fn set_denomination(money: &mut Currency, coin: CoinType, value: u32) {
    match coin {
        CoinType::Copper => money.copper = value,
        CoinType::Silver => money.silver = value,
        CoinType::Gold => money.gold = value,
        CoinType::Platinum => money.platinum = value,
    }
}
impl CommerceState {
    fn balance(&self, carried: Currency, location: CoinLocation, coin: CoinType) -> u32 {
        match location {
            CoinLocation::Carried => denomination(carried, coin),
            CoinLocation::Bank => denomination(self.bank_money, coin),
            CoinLocation::SharedBank => self.shared_platinum,
        }
    }
    pub fn validate_coin(
        &self,
        carried: Currency,
        from: CoinLocation,
        to: CoinLocation,
        coin: CoinType,
        amount: u32,
    ) -> Result<(), String> {
        if from == to || amount == 0 || amount > i32::MAX as u32 {
            return Err("Choose an amount and a different money location.".into());
        }
        if (from == CoinLocation::SharedBank || to == CoinLocation::SharedBank)
            && (!self.shared_coin_enabled || coin != CoinType::Platinum)
        {
            return Err("Shared platinum is unavailable on this server.".into());
        }
        if self.balance(carried, from, coin) < amount {
            return Err("There are not enough coins of that denomination.".into());
        }
        if self
            .balance(carried, to, coin)
            .checked_add(amount)
            .is_none()
        {
            return Err("The destination cannot hold that many coins.".into());
        }
        Ok(())
    }
    pub fn move_coins(
        &mut self,
        carried: &mut Currency,
        from: CoinLocation,
        to: CoinLocation,
        coin: CoinType,
        amount: u32,
    ) -> Result<(), String> {
        self.validate_coin(*carried, from, to, coin, amount)?;
        let old = self.balance(*carried, from, coin) - amount;
        let new = self.balance(*carried, to, coin) + amount;
        for (location, value) in [(from, old), (to, new)] {
            match location {
                CoinLocation::Carried => set_denomination(carried, coin, value),
                CoinLocation::Bank => set_denomination(&mut self.bank_money, coin, value),
                CoinLocation::SharedBank => self.shared_platinum = value,
            }
        }
        Ok(())
    }
}

pub fn in_range(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().chain(&b).all(|x| x.is_finite())
        && a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f32>()
            <= SERVICE_RANGE * SERVICE_RANGE
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn purchases_convert_denominations_without_underflow_or_double_spend() {
        let mut money = Currency {
            platinum: 2,
            copper: 7,
            ..Default::default()
        };
        assert!(debit(&mut money, 1118));
        assert_eq!(total_copper(money), 889);
        assert_eq!((money.gold, money.silver, money.copper), (8, 8, 9));
        assert!(!debit(&mut money, 890));
        assert_eq!(total_copper(money), 889);
        assert!(
            total_copper(Currency {
                platinum: u32::MAX,
                ..Default::default()
            }) > u64::from(u32::MAX)
        );
    }
    #[test]
    fn coin_moves_are_checked_and_purchase_keeps_higher_denominations() {
        let mut carried = Currency {
            platinum: 99,
            gold: 8,
            silver: 18,
            copper: 12,
        };
        assert!(debit(&mut carried, 10));
        assert_eq!(
            (
                carried.platinum,
                carried.gold,
                carried.silver,
                carried.copper
            ),
            (99, 8, 18, 2)
        );
        let mut state = CommerceState::default();
        assert!(
            state
                .move_coins(
                    &mut carried,
                    CoinLocation::Carried,
                    CoinLocation::SharedBank,
                    CoinType::Platinum,
                    1
                )
                .is_err()
        );
        state.shared_coin_enabled = true;
        state
            .move_coins(
                &mut carried,
                CoinLocation::Carried,
                CoinLocation::SharedBank,
                CoinType::Platinum,
                1,
            )
            .unwrap();
        assert_eq!(carried.platinum, 98);
        assert_eq!(state.shared_platinum, 1);
        state
            .move_coins(
                &mut carried,
                CoinLocation::SharedBank,
                CoinLocation::Carried,
                CoinType::Platinum,
                1,
            )
            .unwrap();
        assert_eq!(carried.platinum, 99);
        assert_eq!(state.shared_platinum, 0);
        assert!(
            state
                .move_coins(
                    &mut carried,
                    CoinLocation::Carried,
                    CoinLocation::Bank,
                    CoinType::Copper,
                    3
                )
                .is_err()
        );
        assert_eq!(carried.copper, 2);
    }

    #[test]
    fn service_access_rejects_out_of_range_and_invalid_positions() {
        assert!(in_range([0.; 3], [0., 0., 200.]));
        assert!(!in_range([0.; 3], [200., 1., 0.]));
        assert!(!in_range([0.; 3], [f32::NAN, 0., 0.]));
    }
}
