//! Pure portfolio domain calculations.
//!
//! This module intentionally receives validated transaction records as input and performs no
//! database or UI access. It does not fetch market data or issue trading instructions.

use std::{error::Error, fmt};

use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityType {
    Stock,
    Etf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRule {
    TPlus1,
    TPlus0,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Confirmed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioTransaction {
    pub id: i64,
    /// ISO 8601 trading date (`YYYY-MM-DD`) in the Shanghai market calendar.
    pub trade_date: String,
    pub side: TransactionSide,
    pub status: TransactionStatus,
    pub quantity: i64,
    pub price: Decimal,
    /// The actual charged commission, which may have been determined by a minimum commission rule.
    pub commission: Decimal,
    pub stamp_tax: Decimal,
    pub transfer_fee: Decimal,
    pub other_fee: Decimal,
    /// The minimum-commission setting recorded with the transaction; it is metadata, not a second fee.
    pub minimum_commission: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub quantity: i64,
    /// `None` means the instrument's trade rule is unknown, so sellability cannot be calculated.
    pub available_quantity: Option<i64>,
    pub average_cost: Decimal,
    pub cost_amount: Decimal,
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionValuation {
    pub market_value: Decimal,
    pub unrealized_pnl: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioError {
    InvalidQuantity {
        transaction_id: i64,
    },
    InvalidPrice {
        transaction_id: i64,
    },
    InvalidFee {
        transaction_id: i64,
    },
    InvalidTradeDate {
        transaction_id: i64,
        trade_date: String,
    },
    InsufficientPosition {
        transaction_id: i64,
        requested: i64,
        held: i64,
    },
    InsufficientAvailableQuantity {
        transaction_id: i64,
        requested: i64,
        available: i64,
    },
    TradeRuleUnknown,
    InvalidAvailableQuantity,
}

impl fmt::Display for PortfolioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuantity { transaction_id } => {
                write!(formatter, "transaction {transaction_id} has an invalid quantity")
            }
            Self::InvalidPrice { transaction_id } => {
                write!(formatter, "transaction {transaction_id} has an invalid price")
            }
            Self::InvalidFee { transaction_id } => {
                write!(formatter, "transaction {transaction_id} has an invalid fee")
            }
            Self::InvalidTradeDate {
                transaction_id,
                trade_date,
            } => write!(
                formatter,
                "transaction {transaction_id} has an invalid trading date: {trade_date}"
            ),
            Self::InsufficientPosition {
                transaction_id,
                requested,
                held,
            } => write!(
                formatter,
                "transaction {transaction_id} sells {requested} shares but only {held} are held"
            ),
            Self::InsufficientAvailableQuantity {
                transaction_id,
                requested,
                available,
            } => write!(
                formatter,
                "transaction {transaction_id} sells {requested} shares but only {available} are available"
            ),
            Self::TradeRuleUnknown => write!(formatter, "sellable quantity is unavailable for UNKNOWN trade rule"),
            Self::InvalidAvailableQuantity => write!(formatter, "available quantity is invalid"),
        }
    }
}

impl Error for PortfolioError {}

/// Application service for one account + security position.
#[derive(Debug, Clone, Copy)]
pub struct PortfolioService {
    security_type: SecurityType,
    trade_rule: TradeRule,
}

impl PortfolioService {
    pub fn new(security_type: SecurityType, trade_rule: TradeRule) -> Self {
        Self {
            security_type,
            trade_rule,
        }
    }

    /// Returns the portfolio state after applying all confirmed transactions in date/id order.
    ///
    /// This is a calculation-only service: it does not persist results or change ledger records.
    pub fn calculate(
        &self,
        transactions: &[PortfolioTransaction],
    ) -> Result<Position, PortfolioError> {
        let mut confirmed: Vec<&PortfolioTransaction> = transactions
            .iter()
            .filter(|transaction| transaction.status == TransactionStatus::Confirmed)
            .collect();
        confirmed.sort_by(|left, right| {
            left.trade_date
                .cmp(&right.trade_date)
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut state = CalculationState::new(self.trade_rule);
        for transaction in confirmed {
            validate(transaction)?;
            state.release_t_plus_one_quantity(&transaction.trade_date);

            match transaction.side {
                TransactionSide::Buy => state.apply_buy(transaction),
                TransactionSide::Sell => state.apply_sell(transaction)?,
            }
        }

        Ok(state.into_position())
    }

    /// Calculates market value and floating P&L using a caller-provided, already validated price.
    pub fn value_position(&self, position: &Position, current_price: Decimal) -> PositionValuation {
        let quantity = Decimal::from(position.quantity);
        let market_value = quantity * current_price;
        PositionValuation {
            market_value,
            unrealized_pnl: market_value - position.cost_amount,
        }
    }

    /// Builds a position from a user-entered opening balance. This is a local ledger position,
    /// not an order or a simulated market transaction. The caller owns audit metadata and storage.
    pub fn position_from_opening_balance(
        &self,
        quantity: i64,
        available_quantity: i64,
        average_cost: Decimal,
    ) -> Result<Position, PortfolioError> {
        if quantity <= 0 {
            return Err(PortfolioError::InvalidQuantity { transaction_id: 0 });
        }
        if average_cost.is_sign_negative() {
            return Err(PortfolioError::InvalidPrice { transaction_id: 0 });
        }
        if available_quantity < 0 || available_quantity > quantity {
            return Err(PortfolioError::InvalidAvailableQuantity);
        }
        Ok(Position {
            quantity,
            available_quantity: (self.trade_rule != TradeRule::Unknown)
                .then_some(available_quantity),
            average_cost,
            cost_amount: Decimal::from(quantity) * average_cost,
            realized_pnl: Decimal::ZERO,
        })
    }

    /// Today's price-only P&L uses the source-backed previous close. Fees and realized P&L are
    /// intentionally excluded, matching the V1.0 daily P&L definition.
    pub fn daily_pnl(
        &self,
        position: &Position,
        current_price: Decimal,
        previous_close: Decimal,
    ) -> Decimal {
        Decimal::from(position.quantity) * (current_price - previous_close)
    }

    /// Applies the configured lower bound when an upstream brokerage-rate calculation needs it.
    /// The result is the charge to persist in `commission`; it is never added twice.
    pub fn commission_with_minimum(
        &self,
        rate_calculated_commission: Decimal,
        minimum_commission: Decimal,
    ) -> Decimal {
        rate_calculated_commission.max(minimum_commission)
    }

    pub fn security_type(&self) -> SecurityType {
        self.security_type
    }
}

#[derive(Debug)]
struct PendingBuy {
    trade_date: String,
    quantity: i64,
}

#[derive(Debug)]
struct CalculationState {
    trade_rule: TradeRule,
    quantity: i64,
    available_quantity: i64,
    average_cost: Decimal,
    cost_amount: Decimal,
    realized_pnl: Decimal,
    pending_buys: Vec<PendingBuy>,
}

impl CalculationState {
    fn new(trade_rule: TradeRule) -> Self {
        Self {
            trade_rule,
            quantity: 0,
            available_quantity: 0,
            average_cost: Decimal::ZERO,
            cost_amount: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            pending_buys: Vec::new(),
        }
    }

    fn release_t_plus_one_quantity(&mut self, current_date: &str) {
        if self.trade_rule != TradeRule::TPlus1 {
            return;
        }

        let mut still_pending = Vec::with_capacity(self.pending_buys.len());
        for pending_buy in self.pending_buys.drain(..) {
            if pending_buy.trade_date.as_str() < current_date {
                self.available_quantity += pending_buy.quantity;
            } else {
                still_pending.push(pending_buy);
            }
        }
        self.pending_buys = still_pending;
    }

    fn apply_buy(&mut self, transaction: &PortfolioTransaction) {
        let quantity = Decimal::from(transaction.quantity);
        self.cost_amount += transaction.price * quantity + buy_fees(transaction);
        self.quantity += transaction.quantity;
        self.average_cost = self.cost_amount / Decimal::from(self.quantity);

        match self.trade_rule {
            TradeRule::TPlus1 => self.pending_buys.push(PendingBuy {
                trade_date: transaction.trade_date.clone(),
                quantity: transaction.quantity,
            }),
            TradeRule::TPlus0 => self.available_quantity += transaction.quantity,
            TradeRule::Unknown => {}
        }
    }

    fn apply_sell(&mut self, transaction: &PortfolioTransaction) -> Result<(), PortfolioError> {
        if self.trade_rule == TradeRule::Unknown {
            return Err(PortfolioError::TradeRuleUnknown);
        }
        if transaction.quantity > self.quantity {
            return Err(PortfolioError::InsufficientPosition {
                transaction_id: transaction.id,
                requested: transaction.quantity,
                held: self.quantity,
            });
        }
        if transaction.quantity > self.available_quantity {
            return Err(PortfolioError::InsufficientAvailableQuantity {
                transaction_id: transaction.id,
                requested: transaction.quantity,
                available: self.available_quantity,
            });
        }

        let quantity = Decimal::from(transaction.quantity);
        let matched_cost = self.average_cost * quantity;
        let sale_proceeds = transaction.price * quantity;
        self.realized_pnl += sale_proceeds - matched_cost - sell_fees(transaction);
        self.quantity -= transaction.quantity;
        self.available_quantity -= transaction.quantity;
        self.cost_amount -= matched_cost;

        if self.quantity == 0 {
            self.average_cost = Decimal::ZERO;
            self.cost_amount = Decimal::ZERO;
        }

        Ok(())
    }

    fn into_position(self) -> Position {
        Position {
            quantity: self.quantity,
            available_quantity: (self.trade_rule != TradeRule::Unknown)
                .then_some(self.available_quantity),
            average_cost: self.average_cost,
            cost_amount: self.cost_amount,
            realized_pnl: self.realized_pnl,
        }
    }
}

fn buy_fees(transaction: &PortfolioTransaction) -> Decimal {
    transaction.commission + transaction.transfer_fee + transaction.other_fee
}

fn sell_fees(transaction: &PortfolioTransaction) -> Decimal {
    transaction.commission
        + transaction.stamp_tax
        + transaction.transfer_fee
        + transaction.other_fee
}

fn validate(transaction: &PortfolioTransaction) -> Result<(), PortfolioError> {
    if transaction.quantity <= 0 {
        return Err(PortfolioError::InvalidQuantity {
            transaction_id: transaction.id,
        });
    }
    if transaction.price.is_sign_negative() {
        return Err(PortfolioError::InvalidPrice {
            transaction_id: transaction.id,
        });
    }
    if [
        transaction.commission,
        transaction.stamp_tax,
        transaction.transfer_fee,
        transaction.other_fee,
        transaction.minimum_commission,
    ]
    .iter()
    .any(Decimal::is_sign_negative)
    {
        return Err(PortfolioError::InvalidFee {
            transaction_id: transaction.id,
        });
    }
    if !is_iso_trade_date(&transaction.trade_date) {
        return Err(PortfolioError::InvalidTradeDate {
            transaction_id: transaction.id,
            trade_date: transaction.trade_date.clone(),
        });
    }
    Ok(())
}

fn is_iso_trade_date(date: &str) -> bool {
    date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("valid decimal test value")
    }

    fn transaction(
        id: i64,
        date: &str,
        side: TransactionSide,
        quantity: i64,
        price: &str,
    ) -> PortfolioTransaction {
        PortfolioTransaction {
            id,
            trade_date: date.into(),
            side,
            status: TransactionStatus::Confirmed,
            quantity,
            price: decimal(price),
            commission: Decimal::ZERO,
            stamp_tax: Decimal::ZERO,
            transfer_fee: Decimal::ZERO,
            other_fee: Decimal::ZERO,
            minimum_commission: Decimal::ZERO,
        }
    }

    fn stock_service() -> PortfolioService {
        PortfolioService::new(SecurityType::Stock, TradeRule::TPlus1)
    }

    #[test]
    fn single_buy_calculates_quantity_cost_and_unrealized_pnl() {
        let engine = stock_service();
        let position = engine
            .calculate(&[transaction(
                1,
                "2026-08-03",
                TransactionSide::Buy,
                100,
                "10",
            )])
            .expect("calculate single buy");
        let valuation = engine.value_position(&position, decimal("12"));

        assert_eq!(position.quantity, 100);
        assert_eq!(position.available_quantity, Some(0));
        assert_eq!(position.average_cost, decimal("10"));
        assert_eq!(position.cost_amount, decimal("1000"));
        assert_eq!(position.realized_pnl, Decimal::ZERO);
        assert_eq!(valuation.market_value, decimal("1200"));
        assert_eq!(valuation.unrealized_pnl, decimal("200"));
    }

    #[test]
    fn multiple_buys_use_moving_average_cost() {
        let engine = stock_service();
        let position = engine
            .calculate(&[
                transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10"),
                transaction(2, "2026-08-04", TransactionSide::Buy, 100, "14"),
            ])
            .expect("calculate average cost");
        let valuation = engine.value_position(&position, decimal("13"));

        assert_eq!(position.quantity, 200);
        assert_eq!(position.available_quantity, Some(100));
        assert_eq!(position.average_cost, decimal("12"));
        assert_eq!(position.cost_amount, decimal("2400"));
        assert_eq!(position.realized_pnl, Decimal::ZERO);
        assert_eq!(valuation.unrealized_pnl, decimal("200"));
    }

    #[test]
    fn partial_sell_preserves_average_cost_and_calculates_realized_pnl() {
        let engine = stock_service();
        let position = engine
            .calculate(&[
                transaction(1, "2026-08-03", TransactionSide::Buy, 200, "10"),
                transaction(2, "2026-08-04", TransactionSide::Sell, 50, "12"),
            ])
            .expect("calculate partial sell");
        let valuation = engine.value_position(&position, decimal("11"));

        assert_eq!(position.quantity, 150);
        assert_eq!(position.available_quantity, Some(150));
        assert_eq!(position.average_cost, decimal("10"));
        assert_eq!(position.cost_amount, decimal("1500"));
        assert_eq!(position.realized_pnl, decimal("100"));
        assert_eq!(valuation.unrealized_pnl, decimal("150"));
    }

    #[test]
    fn full_sell_resets_open_position_and_keeps_realized_pnl() {
        let engine = stock_service();
        let position = engine
            .calculate(&[
                transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10"),
                transaction(2, "2026-08-04", TransactionSide::Sell, 100, "12"),
            ])
            .expect("calculate full sell");
        let valuation = engine.value_position(&position, decimal("12"));

        assert_eq!(position.quantity, 0);
        assert_eq!(position.available_quantity, Some(0));
        assert_eq!(position.average_cost, Decimal::ZERO);
        assert_eq!(position.cost_amount, Decimal::ZERO);
        assert_eq!(position.realized_pnl, decimal("200"));
        assert_eq!(valuation.market_value, Decimal::ZERO);
        assert_eq!(valuation.unrealized_pnl, Decimal::ZERO);
    }

    #[test]
    fn fees_are_included_once_in_cost_and_realized_pnl() {
        let engine = stock_service();
        let mut buy = transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10");
        buy.commission = decimal("5");
        buy.transfer_fee = decimal("1");
        buy.other_fee = decimal("2");
        buy.minimum_commission = decimal("5");

        let mut sell = transaction(2, "2026-08-04", TransactionSide::Sell, 100, "11");
        sell.commission = decimal("5");
        sell.stamp_tax = decimal("1");
        sell.transfer_fee = decimal("1");
        sell.other_fee = decimal("1");
        sell.minimum_commission = decimal("5");

        let position = engine.calculate(&[buy, sell]).expect("calculate fees");
        let valuation = engine.value_position(&position, decimal("11"));

        assert_eq!(
            engine.commission_with_minimum(decimal("3"), decimal("5")),
            decimal("5")
        );
        assert_eq!(position.quantity, 0);
        assert_eq!(position.cost_amount, Decimal::ZERO);
        assert_eq!(position.realized_pnl, decimal("84"));
        assert_eq!(valuation.unrealized_pnl, Decimal::ZERO);
    }

    #[test]
    fn cancelled_transaction_is_ignored() {
        let engine = stock_service();
        let mut cancelled = transaction(2, "2026-08-04", TransactionSide::Sell, 100, "20");
        cancelled.status = TransactionStatus::Cancelled;

        let position = engine
            .calculate(&[
                transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10"),
                cancelled,
            ])
            .expect("ignore cancelled transaction");
        let valuation = engine.value_position(&position, decimal("12"));

        assert_eq!(position.quantity, 100);
        assert_eq!(position.cost_amount, decimal("1000"));
        assert_eq!(position.realized_pnl, Decimal::ZERO);
        assert_eq!(valuation.unrealized_pnl, decimal("200"));
    }

    #[test]
    fn t_plus_one_blocks_same_day_sale_and_allows_later_trade_day() {
        let engine = stock_service();
        let same_day = [
            transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10"),
            transaction(2, "2026-08-03", TransactionSide::Sell, 100, "11"),
        ];
        assert_eq!(
            engine.calculate(&same_day),
            Err(PortfolioError::InsufficientAvailableQuantity {
                transaction_id: 2,
                requested: 100,
                available: 0,
            })
        );

        let later_day = [
            transaction(1, "2026-08-03", TransactionSide::Buy, 100, "10"),
            transaction(2, "2026-08-04", TransactionSide::Sell, 100, "11"),
        ];
        let position = engine
            .calculate(&later_day)
            .expect("allow next trade day sale");
        let valuation = engine.value_position(&position, decimal("11"));
        assert_eq!(position.quantity, 0);
        assert_eq!(position.cost_amount, Decimal::ZERO);
        assert_eq!(position.realized_pnl, decimal("100"));
        assert_eq!(valuation.unrealized_pnl, Decimal::ZERO);
    }

    #[test]
    fn etf_unknown_rule_disables_available_quantity_and_sell_calculation() {
        let engine = PortfolioService::new(SecurityType::Etf, TradeRule::Unknown);
        let buy = transaction(1, "2026-08-03", TransactionSide::Buy, 100, "3.85");
        let position = engine
            .calculate(&[buy.clone()])
            .expect("calculate ETF holding");
        let valuation = engine.value_position(&position, decimal("4.00"));

        assert_eq!(position.quantity, 100);
        assert_eq!(position.available_quantity, None);
        assert_eq!(position.cost_amount, decimal("385"));
        assert_eq!(position.realized_pnl, Decimal::ZERO);
        assert_eq!(valuation.unrealized_pnl, decimal("15"));

        let sell = transaction(2, "2026-08-04", TransactionSide::Sell, 100, "4.00");
        assert_eq!(
            engine.calculate(&[buy, sell]),
            Err(PortfolioError::TradeRuleUnknown)
        );
    }

    #[test]
    fn opening_balance_and_daily_pnl_are_calculated_in_rust() {
        let engine = stock_service();
        let position = engine
            .position_from_opening_balance(200, 200, decimal("10.50"))
            .expect("valid opening balance");
        let valuation = engine.value_position(&position, decimal("11.20"));

        assert_eq!(position.cost_amount, decimal("2100"));
        assert_eq!(position.available_quantity, Some(200));
        assert_eq!(valuation.market_value, decimal("2240"));
        assert_eq!(valuation.unrealized_pnl, decimal("140"));
        assert_eq!(
            engine.daily_pnl(&position, decimal("11.20"), decimal("11.00")),
            decimal("40")
        );
    }
}
