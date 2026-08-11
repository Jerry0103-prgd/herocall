//! Read/write application service for the local holdings screen.
//!
//! The UI only sends user input and renders these views. Exact cost, market value, floating P&L
//! and daily P&L calculations happen in Rust through the Portfolio and Market service contracts.

use std::{error::Error, fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    database::service::{
        DatabaseError, DatabaseService, HoldingUpdate, NewHolding, NewSecurity,
        PortfolioHoldingData, WatchlistItemData,
    },
    market_service::{MarketService, MarketStatus},
    portfolio_service::{PortfolioService, SecurityType, TradeRule},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateHoldingInput {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub security_type: String,
    pub quantity: String,
    pub average_cost: String,
}

/// A research watchlist record intentionally has no quantity, cost, or transaction data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWatchlistInput {
    pub symbol: String,
    pub name: String,
}

/// Exact identity for a user-managed follow. Keeping the watchlist row id in the command
/// contract prevents an unrelated historical position from ever being treated as the follow.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWatchlistInput {
    pub watchlist_item_id: i64,
    pub security_id: i64,
}

/// A local, source-backed security candidate. This is intentionally limited to persisted
/// securities so the UI never invents a code/name pairing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityLookupView {
    pub security_id: i64,
    pub symbol: String,
    pub name: String,
    pub exchange: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHoldingInput {
    pub holding_id: i64,
    pub name: String,
    pub quantity: String,
    pub average_cost: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioHoldingView {
    pub holding_id: i64,
    pub security_id: i64,
    pub name: String,
    pub symbol: String,
    pub market: String,
    pub security_type: String,
    pub quantity: String,
    pub available_quantity: Option<String>,
    pub average_cost: String,
    pub current_price: Option<String>,
    pub market_value: Option<String>,
    pub daily_pnl: Option<String>,
    pub total_pnl: Option<String>,
    pub change_percent: Option<String>,
    pub transaction_status: String,
    pub is_watchlist: bool,
    pub profile_status: Option<String>,
}

#[derive(Debug)]
pub enum PortfolioUiError {
    Database(DatabaseError),
    Validation(&'static str),
    ExistingHolding,
    MissingHolding,
}

impl fmt::Display for PortfolioUiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::ExistingHolding => formatter.write_str("该证券已在我的关注中"),
            Self::MissingHolding => formatter.write_str("未找到该持仓"),
        }
    }
}

impl Error for PortfolioUiError {}

impl From<DatabaseError> for PortfolioUiError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub struct PortfolioUiService;

impl PortfolioUiService {
    pub fn list(database: &DatabaseService) -> Result<Vec<PortfolioHoldingView>, PortfolioUiError> {
        database
            .list_portfolio_holding_data()?
            .into_iter()
            .map(Self::to_view)
            .collect()
    }

    pub fn list_watchlist(
        database: &DatabaseService,
    ) -> Result<Vec<PortfolioHoldingView>, PortfolioUiError> {
        database
            .list_watchlist_item_data()?
            .into_iter()
            .map(Self::watchlist_view)
            .collect()
    }

    pub fn search_securities(
        database: &DatabaseService,
        query: &str,
    ) -> Result<Vec<SecurityLookupView>, PortfolioUiError> {
        database
            .search_securities(query, 12)?
            .into_iter()
            .map(|security| {
                Ok(SecurityLookupView {
                    security_id: security.id,
                    symbol: security.symbol,
                    name: security.name,
                    exchange: security.exchange,
                })
            })
            .collect()
    }

    pub fn create(
        database: &DatabaseService,
        input: CreateHoldingInput,
    ) -> Result<PortfolioHoldingView, PortfolioUiError> {
        let parsed = ParsedCreateInput::parse(input)?;
        let account = database.get_or_create_local_holding_account()?;
        let security =
            match database.find_security_by_symbol_and_market(&parsed.symbol, &parsed.market)? {
                Some(security) => database.update_security_for_portfolio(
                    security.id,
                    &parsed.name,
                    &parsed.security_type,
                    &parsed.trade_rule,
                )?,
                None => database.create_security(NewSecurity {
                    symbol: parsed.symbol.clone(),
                    name: parsed.name.clone(),
                    market: parsed.market.clone(),
                    exchange: parsed.market.clone(),
                    security_type: parsed.security_type.clone(),
                    industry: None,
                    concepts_json: "[]".into(),
                    trade_rule: parsed.trade_rule.clone(),
                })?,
            };
        if database
            .find_holding_by_account_and_security(account.id, security.id)?
            .is_some()
        {
            return Err(PortfolioUiError::ExistingHolding);
        }

        let service =
            PortfolioService::new(parsed.portfolio_security_type, parsed.trade_rule_value);
        let available_quantity = if parsed.trade_rule_value == TradeRule::Unknown {
            0
        } else {
            parsed.quantity
        };
        let position = service
            .position_from_opening_balance(parsed.quantity, available_quantity, parsed.average_cost)
            .map_err(|_| PortfolioUiError::Validation("持仓数量、可卖数量或成本价无效"))?;
        let holding = database.create_holding(NewHolding {
            cash_account_id: account.id,
            security_id: security.id,
            quantity: position.quantity,
            available_quantity,
            average_cost: position.average_cost.to_string(),
            cost_amount: position.cost_amount.to_string(),
            position_source: "MANUAL".into(),
            as_of_date: None,
        })?;
        Self::list(database)?
            .into_iter()
            .find(|view| view.holding_id == holding.id)
            .ok_or(PortfolioUiError::MissingHolding)
    }

    /// Creates a user-managed follow without requiring a provider-backed local security master.
    /// When the code is not already known locally, only the user's code and name are persisted;
    /// market, exchange and trading rule remain explicitly `UNKNOWN`.
    pub fn create_watchlist(
        database: &DatabaseService,
        input: CreateWatchlistInput,
    ) -> Result<PortfolioHoldingView, PortfolioUiError> {
        let symbol = normalized_symbol(&input.symbol)?;
        let name = normalized_name(&input.name)?;
        let security = match database.find_security_by_symbol(&symbol)? {
            Some(existing) => existing,
            None => database.create_security(NewSecurity {
                symbol,
                name,
                market: "UNKNOWN".into(),
                exchange: "UNKNOWN".into(),
                // The user did not provide a type. This required schema default does not imply
                // provider verification; the market and trade rule remain explicit unknowns.
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "UNKNOWN".into(),
            })?,
        };
        if database
            .find_watchlist_item_by_security(security.id)?
            .is_some()
        {
            return Err(PortfolioUiError::ExistingHolding);
        }

        let watchlist_item = database.create_watchlist_item(security.id)?;
        // Profile creation is intentionally non-blocking and contains no inferred company data.
        // A newly followed security is usable even while its profile remains PENDING.
        database.ensure_security_profile_pending(security.id)?;
        Ok(Self::watchlist_view(watchlist_item)?)
    }

    pub fn update(
        database: &DatabaseService,
        input: UpdateHoldingInput,
    ) -> Result<PortfolioHoldingView, PortfolioUiError> {
        let holding = database
            .get_holding(input.holding_id)
            .map_err(|error| match error {
                DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    PortfolioUiError::MissingHolding
                }
                other => PortfolioUiError::Database(other),
            })?;
        let security = database.get_security(holding.security_id)?;
        let name = normalized_name(&input.name)?;
        let quantity = parse_positive_quantity(&input.quantity)?;
        let average_cost = parse_positive_decimal(&input.average_cost, "成本价必须大于 0")?;
        let trade_rule_value = parse_trade_rule(&security.trade_rule)?;
        let service = PortfolioService::new(
            parse_security_type(&security.security_type)?,
            trade_rule_value,
        );
        let available_quantity = if trade_rule_value == TradeRule::Unknown {
            0
        } else {
            quantity
        };
        let position = service
            .position_from_opening_balance(quantity, available_quantity, average_cost)
            .map_err(|_| PortfolioUiError::Validation("持仓数据无效"))?;
        database.update_security_for_portfolio(
            security.id,
            &name,
            &security.security_type,
            &security.trade_rule,
        )?;
        database.update_holding(
            holding.id,
            HoldingUpdate {
                quantity: position.quantity,
                available_quantity,
                average_cost: position.average_cost.to_string(),
                cost_amount: position.cost_amount.to_string(),
                as_of_date: holding.as_of_date,
            },
        )?;
        Self::list(database)?
            .into_iter()
            .find(|view| view.holding_id == holding.id)
            .ok_or(PortfolioUiError::MissingHolding)
    }

    pub fn delete(database: &DatabaseService, holding_id: i64) -> Result<(), PortfolioUiError> {
        if database.delete_holding(holding_id)? == 0 {
            return Err(PortfolioUiError::MissingHolding);
        }
        Ok(())
    }

    pub fn remove_followed_security_completely(
        database: &DatabaseService,
        input: DeleteWatchlistInput,
    ) -> Result<(), PortfolioUiError> {
        database.remove_followed_security_completely(input.watchlist_item_id, input.security_id)?;
        Ok(())
    }

    fn watchlist_view(data: WatchlistItemData) -> Result<PortfolioHoldingView, PortfolioUiError> {
        Ok(PortfolioHoldingView {
            holding_id: data.watchlist_item_id,
            security_id: data.security_id,
            name: data.name,
            symbol: data.symbol,
            market: data.market,
            security_type: data.security_type,
            quantity: "0".into(),
            available_quantity: None,
            average_cost: "0".into(),
            current_price: data.current_price,
            market_value: None,
            daily_pnl: None,
            total_pnl: None,
            change_percent: data.change_percent,
            transaction_status: "关注标的".into(),
            is_watchlist: true,
            profile_status: Some(data.profile_status),
        })
    }

    fn to_view(data: PortfolioHoldingData) -> Result<PortfolioHoldingView, PortfolioUiError> {
        if data.quantity == 0 {
            return Ok(PortfolioHoldingView {
                holding_id: data.holding_id,
                security_id: data.security_id,
                name: data.name,
                symbol: data.symbol,
                market: data.market,
                security_type: data.security_type,
                quantity: "0".into(),
                available_quantity: Some("0".into()),
                average_cost: "0".into(),
                current_price: data.current_price,
                market_value: None,
                daily_pnl: None,
                total_pnl: None,
                change_percent: data.change_percent,
                transaction_status: "关注标的".into(),
                is_watchlist: true,
                profile_status: None,
            });
        }
        let security_type = parse_security_type(&data.security_type)?;
        let trade_rule = parse_trade_rule(&data.trade_rule)?;
        let service = PortfolioService::new(security_type, trade_rule);
        let average_cost = Decimal::from_str(&data.average_cost)
            .map_err(|_| PortfolioUiError::Validation("持仓成本数据未确认"))?;
        let position = service
            .position_from_opening_balance(data.quantity, data.available_quantity, average_cost)
            .map_err(|_| PortfolioUiError::Validation("持仓数量数据未确认"))?;
        let quote_status = data.quote_status.as_deref().and_then(parse_market_status);
        let current_price = data
            .current_price
            .as_deref()
            .and_then(|value| Decimal::from_str(value).ok());
        let previous_close = data
            .previous_close
            .as_deref()
            .and_then(|value| Decimal::from_str(value).ok());
        let is_usable_quote = quote_status
            .map(MarketService::is_usable_for_valuation)
            .unwrap_or(false);
        let valuation = current_price
            .filter(|_| is_usable_quote)
            .map(|price| service.value_position(&position, price));
        let daily_pnl = match (current_price, previous_close, is_usable_quote) {
            (Some(current_price), Some(previous_close), true) => Some(
                service
                    .daily_pnl(&position, current_price, previous_close)
                    .to_string(),
            ),
            _ => None,
        };

        Ok(PortfolioHoldingView {
            holding_id: data.holding_id,
            security_id: data.security_id,
            name: data.name,
            symbol: data.symbol,
            market: data.market,
            security_type: data.security_type,
            quantity: data.quantity.to_string(),
            available_quantity: position
                .available_quantity
                .map(|quantity| quantity.to_string()),
            average_cost: position.average_cost.to_string(),
            current_price: current_price.map(|price| price.to_string()),
            market_value: valuation
                .as_ref()
                .map(|value| value.market_value.to_string()),
            daily_pnl,
            // Total P&L requires the complete confirmed ledger, including realized P&L. A manual
            // holdings record alone is insufficient, so it must remain unavailable.
            total_pnl: None,
            change_percent: data.change_percent,
            transaction_status: data
                .transaction_status
                .unwrap_or_else(|| "暂无交易记录".into()),
            is_watchlist: false,
            profile_status: None,
        })
    }
}

struct ParsedCreateInput {
    symbol: String,
    name: String,
    market: String,
    security_type: String,
    trade_rule: String,
    portfolio_security_type: SecurityType,
    trade_rule_value: TradeRule,
    quantity: i64,
    average_cost: Decimal,
}

impl ParsedCreateInput {
    fn parse(input: CreateHoldingInput) -> Result<Self, PortfolioUiError> {
        let symbol = input.symbol.trim().to_owned();
        if symbol.len() != 6 || !symbol.bytes().all(|value| value.is_ascii_digit()) {
            return Err(PortfolioUiError::Validation("证券代码必须是 6 位数字"));
        }
        let market = match input.market.as_str() {
            "SSE" | "SZSE" => input.market,
            _ => return Err(PortfolioUiError::Validation("市场必须是 SSE 或 SZSE")),
        };
        let security_type = match input.security_type.as_str() {
            "STOCK" | "ETF" => input.security_type,
            _ => return Err(PortfolioUiError::Validation("证券类型必须是 STOCK 或 ETF")),
        };
        let portfolio_security_type = parse_security_type(&security_type)?;
        let trade_rule_value = match portfolio_security_type {
            SecurityType::Stock => TradeRule::TPlus1,
            SecurityType::Etf => TradeRule::Unknown,
        };
        let trade_rule = match trade_rule_value {
            TradeRule::TPlus1 => "T_PLUS_1",
            TradeRule::TPlus0 => "T_PLUS_0",
            TradeRule::Unknown => "UNKNOWN",
        }
        .into();
        Ok(Self {
            symbol,
            name: normalized_name(&input.name)?,
            market,
            security_type,
            trade_rule,
            portfolio_security_type,
            trade_rule_value,
            quantity: parse_positive_quantity(&input.quantity)?,
            average_cost: parse_positive_decimal(&input.average_cost, "成本价必须大于 0")?,
        })
    }
}

fn normalized_name(value: &str) -> Result<String, PortfolioUiError> {
    let value = value.trim();
    (!value.is_empty())
        .then_some(value.to_owned())
        .ok_or(PortfolioUiError::Validation("证券名称不能为空"))
}

fn normalized_symbol(value: &str) -> Result<String, PortfolioUiError> {
    let value = value.trim();
    (value.len() == 6 && value.bytes().all(|item| item.is_ascii_digit()))
        .then_some(value.to_owned())
        .ok_or(PortfolioUiError::Validation("证券代码必须是 6 位数字"))
}

fn parse_positive_quantity(value: &str) -> Result<i64, PortfolioUiError> {
    value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(PortfolioUiError::Validation("持仓数量必须是正整数"))
}

fn parse_positive_decimal(value: &str, message: &'static str) -> Result<Decimal, PortfolioUiError> {
    Decimal::from_str(value.trim())
        .ok()
        .filter(|value| *value > Decimal::ZERO)
        .ok_or(PortfolioUiError::Validation(message))
}

fn parse_security_type(value: &str) -> Result<SecurityType, PortfolioUiError> {
    match value {
        "STOCK" => Ok(SecurityType::Stock),
        "ETF" => Ok(SecurityType::Etf),
        _ => Err(PortfolioUiError::Validation("证券类型未确认")),
    }
}

fn parse_trade_rule(value: &str) -> Result<TradeRule, PortfolioUiError> {
    match value {
        "T_PLUS_1" => Ok(TradeRule::TPlus1),
        "T_PLUS_0" => Ok(TradeRule::TPlus0),
        "UNKNOWN" => Ok(TradeRule::Unknown),
        _ => Err(PortfolioUiError::Validation("交易规则未确认")),
    }
}

fn parse_market_status(value: &str) -> Option<MarketStatus> {
    match value {
        "REALTIME" => Some(MarketStatus::Realtime),
        "DELAYED" => Some(MarketStatus::Delayed),
        "CLOSED" => Some(MarketStatus::Closed),
        "NO_DATA" => Some(MarketStatus::NoData),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::service::NewTransaction;

    fn create_security(database: &DatabaseService, symbol: &str, name: &str) -> i64 {
        let market = if symbol.starts_with('6') {
            "SSE"
        } else {
            "SZSE"
        };
        database
            .create_security(NewSecurity {
                symbol: symbol.into(),
                name: name.into(),
                market: market.into(),
                exchange: market.into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .expect("create source-backed security")
            .id
    }

    fn create_input() -> CreateHoldingInput {
        CreateHoldingInput {
            symbol: "600519".into(),
            name: "测试持仓证券".into(),
            market: "SSE".into(),
            security_type: "STOCK".into(),
            quantity: "100".into(),
            average_cost: "10.50".into(),
        }
    }

    #[test]
    fn local_holding_crud_uses_rust_services_and_never_invents_market_values() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let created =
            PortfolioUiService::create(&database, create_input()).expect("create holding");
        assert_eq!(created.quantity, "100");
        assert_eq!(created.average_cost, "10.50");
        assert_eq!(created.current_price, None);
        assert_eq!(created.market_value, None);
        assert_eq!(created.daily_pnl, None);
        assert_eq!(created.total_pnl, None);

        let updated = PortfolioUiService::update(
            &database,
            UpdateHoldingInput {
                holding_id: created.holding_id,
                name: "测试持仓证券（已修改）".into(),
                quantity: "200".into(),
                average_cost: "11.00".into(),
            },
        )
        .expect("update holding");
        assert_eq!(updated.name, "测试持仓证券（已修改）");
        assert_eq!(updated.quantity, "200");
        assert_eq!(updated.average_cost, "11.00");

        PortfolioUiService::delete(&database, created.holding_id).expect("delete holding");
        assert!(PortfolioUiService::list(&database)
            .expect("list holdings")
            .is_empty());
    }

    #[test]
    fn watchlist_uses_no_position_or_financial_data() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        create_security(&database, "300209", "测试关注证券");
        let created = PortfolioUiService::create_watchlist(
            &database,
            CreateWatchlistInput {
                symbol: "300209".into(),
                name: "测试关注证券".into(),
            },
        )
        .expect("create watchlist item");

        assert!(created.is_watchlist);
        assert_eq!(created.quantity, "0");
        assert_eq!(created.average_cost, "0");
        assert_eq!(created.current_price, None);
        assert_eq!(created.market_value, None);
        assert_eq!(created.daily_pnl, None);
        assert_eq!(created.total_pnl, None);
        assert_eq!(created.profile_status.as_deref(), Some("PENDING"));
        assert_eq!(
            database
                .list_market_securities_for_holdings()
                .expect("watchlist refresh scope")
                .len(),
            1
        );

        PortfolioUiService::remove_followed_security_completely(
            &database,
            DeleteWatchlistInput {
                watchlist_item_id: created.holding_id,
                security_id: created.security_id,
            },
        )
        .expect("delete watchlist item");
        assert!(database
            .find_watchlist_item_by_security(created.security_id)
            .expect("verify follow was deleted")
            .is_none());
        assert!(PortfolioUiService::list(&database)
            .expect("list watchlist")
            .is_empty());
    }

    #[test]
    fn custom_code_and_name_can_be_followed_without_a_local_security_master_match() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let created = PortfolioUiService::create_watchlist(
            &database,
            CreateWatchlistInput {
                symbol: "123456".into(),
                name: "用户自定义关注".into(),
            },
        )
        .expect("save custom follow");

        assert_eq!(created.symbol, "123456");
        assert_eq!(created.name, "用户自定义关注");
        let security = database
            .find_security_by_symbol("123456")
            .expect("find custom security")
            .expect("custom security stored");
        assert_eq!(security.market, "UNKNOWN");
        assert_eq!(security.exchange, "UNKNOWN");
        assert_eq!(security.industry, None);
        assert_eq!(security.concepts_json, "[]");
    }

    #[test]
    fn removing_a_follow_permanently_deletes_an_existing_position_and_history() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let existing =
            PortfolioUiService::create(&database, create_input()).expect("create holding");

        let follow = PortfolioUiService::create_watchlist(
            &database,
            CreateWatchlistInput {
                symbol: existing.symbol.clone(),
                name: existing.name.clone(),
            },
        )
        .expect("explicitly create follow");
        let persisted_holding = database
            .get_holding(existing.holding_id)
            .expect("existing position");
        database
            .create_transaction(NewTransaction {
                cash_account_id: persisted_holding.cash_account_id,
                security_id: persisted_holding.security_id,
                side: "BUY".into(),
                status: "CONFIRMED".into(),
                record_source: "MANUAL".into(),
                trade_date: "2026-08-10".into(),
                quantity: 100,
                price: "10.50".into(),
                commission: "0".into(),
                stamp_tax: "0".into(),
                transfer_fee: "0".into(),
                other_fee: "0".into(),
                minimum_commission: "0".into(),
                external_reference: None,
                import_batch_id: None,
                note: None,
            })
            .expect("persist historical transaction");
        PortfolioUiService::remove_followed_security_completely(
            &database,
            DeleteWatchlistInput {
                watchlist_item_id: follow.holding_id,
                security_id: follow.security_id,
            },
        )
        .expect("remove follow and all security-owned history");
        assert!(database
            .list_portfolio_holding_data()
            .expect("removed position is absent")
            .is_empty());
        assert!(!database
            .has_confirmed_transactions()
            .expect("removed transaction is absent"));
        assert!(PortfolioUiService::list_watchlist(&database)
            .expect("follow is removed")
            .is_empty());
    }

    #[test]
    fn creating_a_holding_does_not_implicitly_follow_the_security() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let created =
            PortfolioUiService::create(&database, create_input()).expect("create holding record");

        assert!(
            database
                .get_holding(created.holding_id)
                .expect("historical holding remains")
                .id
                > 0
        );
        assert!(PortfolioUiService::list_watchlist(&database)
            .expect("read explicit watchlist")
            .is_empty());
    }

    #[test]
    fn newest_follow_is_listed_first() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        create_security(&database, "600330", "先添加");
        PortfolioUiService::create_watchlist(
            &database,
            CreateWatchlistInput {
                symbol: "600330".into(),
                name: "先添加".into(),
            },
        )
        .expect("first follow");
        create_security(&database, "300209", "后添加");
        PortfolioUiService::create_watchlist(
            &database,
            CreateWatchlistInput {
                symbol: "300209".into(),
                name: "后添加".into(),
            },
        )
        .expect("second follow");

        let follows = PortfolioUiService::list_watchlist(&database).expect("list follows");
        assert_eq!(follows[0].symbol, "300209");
        assert_eq!(follows[1].symbol, "600330");
    }

    #[test]
    fn security_lookup_returns_only_persisted_exact_or_name_matches() {
        let database = DatabaseService::open_in_memory().expect("initialize database");
        let matching_id = create_security(&database, "300209", "行云科技");
        create_security(&database, "600330", "天通股份");

        let by_code =
            PortfolioUiService::search_securities(&database, "300209").expect("search by code");
        assert_eq!(by_code.len(), 1);
        assert_eq!(by_code[0].security_id, matching_id);
        assert_eq!(by_code[0].name, "行云科技");

        let by_name =
            PortfolioUiService::search_securities(&database, "行云").expect("search by name");
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].symbol, "300209");
        assert!(
            PortfolioUiService::search_securities(&database, "不存在的证券")
                .expect("search missing security")
                .is_empty()
        );
    }
}
