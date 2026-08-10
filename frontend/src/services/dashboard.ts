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
  configurationStatus: "CONFIGURED" | "UNCONFIGURED" | "PUBLIC_ONLY";
  status: "REALTIME" | "DELAYED" | "CLOSED" | "NO_DATA";
  quoteCount: number;
  marketTimestamp: string | null;
  fetchedAt: string;
  message: string | null;
};

export type SnapshotSection = {
  source: string;
  status: "REALTIME" | "DELAYED" | "CLOSED" | "NO_DATA";
  itemCount: number;
  updatedAt: string | null;
  message: string | null;
};

export type ManualMarketSnapshot = {
  holdings: MarketRefresh;
  indices: SnapshotSection;
  news: SnapshotSection;
  events: SnapshotSection;
};

export type DataSectionStatus = {
  status: "SYNCED" | "NO_DATA";
  source: string | null;
  itemCount: number;
  updatedAt: string | null;
  delayStatus: string | null;
};

export type DashboardDataStatus = {
  market: DataSectionStatus;
  news: DataSectionStatus;
};

export type MarketIndexQuote = {
  name: string;
  symbol: string;
  lastClose: IndexMetric;
  fiveDayAverage: IndexMetric;
  tenDayAverage: IndexMetric;
  currentPrice: string | null;
  changePercent: string | null;
  openPrice: string | null;
  highPrice: string | null;
  lowPrice: string | null;
  turnoverAmount: string | null;
  source: string | null;
  status: "REALTIME" | "DELAYED" | "CLOSED" | "NO_DATA";
  updatedAt: string | null;
};

export type IndexMetric = {
  price: string | null;
  changePercent: string | null;
};

const noDataIndices: MarketIndexQuote[] = [
  ["上证指数", "000001.SH"],
  ["深证成指", "399001.SZ"],
  ["创业板指", "399006.SZ"],
  ["科创50", "000688.SH"],
].map(([name, symbol]) => ({
  name,
  symbol,
  lastClose: { price: null, changePercent: null },
  fiveDayAverage: { price: null, changePercent: null },
  tenDayAverage: { price: null, changePercent: null },
  currentPrice: null,
  changePercent: null,
  openPrice: null,
  highPrice: null,
  lowPrice: null,
  turnoverAmount: null,
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

export function loadDashboardDataStatus(): Promise<DashboardDataStatus> {
  return invoke<DashboardDataStatus>("get_dashboard_data_status");
}

export function refreshTushareMarketData(): Promise<MarketRefresh> {
  return invoke<MarketRefresh>("refresh_tushare_market_data");
}

export function refreshTodayMarketSnapshot(): Promise<ManualMarketSnapshot> {
  return invoke<ManualMarketSnapshot>("refresh_today_market_snapshot");
}
