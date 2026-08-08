import { invoke } from "@tauri-apps/api/core";

export type AssetSummary = {
  totalAssets: string | null;
  stockMarketValue: string | null;
  cash: string | null;
  dailyPnl: string | null;
  totalPnl: string | null;
  returnRate: string | null;
  valuationSource: string | null;
  valuationTimestamp: string | null;
};

export type MarketRefresh = {
  source: string;
  configurationStatus: "CONFIGURED" | "UNCONFIGURED";
  status: "REALTIME" | "DELAYED" | "CLOSED" | "NO_DATA";
  quoteCount: number;
  marketTimestamp: string | null;
  fetchedAt: string;
  message: string | null;
};

export type MarketIndexQuote = {
  name: string;
  symbol: string;
  currentPrice: string | null;
  changePercent: string | null;
  source: string | null;
  status: "REALTIME" | "DELAYED" | "CLOSED" | "NO_DATA";
  updatedAt: string | null;
};

const noDataIndices: MarketIndexQuote[] = [
  ["上证指数", "000001.SH"],
  ["深证成指", "399001.SZ"],
  ["创业板指", "399006.SZ"],
  ["科创50", "000688.SH"],
].map(([name, symbol]) => ({
  name,
  symbol,
  currentPrice: null,
  changePercent: null,
  source: null,
  status: "NO_DATA" as const,
  updatedAt: null,
}));

export const emptyAssetSummary: AssetSummary = {
  totalAssets: null,
  stockMarketValue: null,
  cash: null,
  dailyPnl: null,
  totalPnl: null,
  returnRate: null,
  valuationSource: null,
  valuationTimestamp: null,
};

export function noDataMarketSnapshot(): MarketIndexQuote[] {
  return noDataIndices.map((quote) => ({ ...quote }));
}

export function loadAssetSummary(): Promise<AssetSummary> {
  return invoke<AssetSummary>("get_asset_summary");
}

export function loadMarketSnapshot(): Promise<MarketIndexQuote[]> {
  return invoke<MarketIndexQuote[]>("get_market_snapshot");
}

export function refreshTushareMarketData(): Promise<MarketRefresh> {
  return invoke<MarketRefresh>("refresh_tushare_market_data");
}
