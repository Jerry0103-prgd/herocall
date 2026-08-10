import { invoke } from "@tauri-apps/api/core";

export type SecurityType = "STOCK" | "ETF";

export type PortfolioHolding = {
  holdingId: number;
  securityId: number;
  name: string;
  symbol: string;
  market: string;
  securityType: SecurityType;
  quantity: string;
  availableQuantity: string | null;
  averageCost: string;
  currentPrice: string | null;
  marketValue: string | null;
  dailyPnl: string | null;
  totalPnl: string | null;
  changePercent: string | null;
  transactionStatus: string;
  isWatchlist: boolean;
};

export type CreateHoldingInput = {
  symbol: string;
  name: string;
  market: "SSE" | "SZSE";
  securityType: SecurityType;
  quantity: string;
  averageCost: string;
};

export type CreateWatchlistInput = {
  symbol: string;
  name: string;
};

export type SecurityLookup = {
  securityId: number;
  symbol: string;
  name: string;
  exchange: string;
};

export type UpdateHoldingInput = {
  holdingId: number;
  name: string;
  quantity: string;
  averageCost: string;
};

export function loadPortfolioHoldings(): Promise<PortfolioHolding[]> {
  return invoke<PortfolioHolding[]>("get_portfolio_holdings");
}

export function createPortfolioHolding(input: CreateHoldingInput): Promise<PortfolioHolding> {
  return invoke<PortfolioHolding>("create_portfolio_holding", { input });
}

export function createWatchlistItem(input: CreateWatchlistInput): Promise<PortfolioHolding> {
  return invoke<PortfolioHolding>("create_watchlist_item", { input });
}

export function searchWatchlistSecurities(query: string): Promise<SecurityLookup[]> {
  return invoke<SecurityLookup[]>("search_watchlist_securities", { query });
}

export function updatePortfolioHolding(input: UpdateHoldingInput): Promise<PortfolioHolding> {
  return invoke<PortfolioHolding>("update_portfolio_holding", { input });
}

export function deletePortfolioHolding(holdingId: number): Promise<void> {
  return invoke<void>("delete_portfolio_holding", { holdingId });
}

export function removeFollowedSecurityCompletely(watchlistItemId: number, securityId: number): Promise<void> {
  return invoke<void>("remove_followed_security_completely", { input: { watchlistItemId, securityId } });
}
