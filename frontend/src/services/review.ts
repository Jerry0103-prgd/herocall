import { invoke } from "@tauri-apps/api/core";

export type ReviewPortfolioSummary = {
  totalAssets: string | null;
  dailyPnl: string | null;
  returnRate: string | null;
  holdingCount: number;
};

export type ReviewMarketIndex = {
  name: string;
  symbol: string;
  changePercent: string | null;
  source: string | null;
  status: string;
  updatedAt: string | null;
};

export type DailyReview = {
  id: number;
  reviewDate: string;
  snapshotId: number | null;
  portfolioSummary: ReviewPortfolioSummary;
  marketSummary: {
    snapshot: { id: number; source: string; marketTimestamp: string; fetchedAt: string; status: string } | null;
    majorIndices: ReviewMarketIndex[];
  };
  holdingSummary: {
    contributions: { name: string; symbol: string; dailyPnl: string | null; changePercent: string | null }[];
  };
  riskSummary: { facts: string[]; relatedNewsCount: number };
  createdAt: string;
};

export function loadDailyReview(reviewDate: string): Promise<DailyReview> {
  return invoke<DailyReview>("get_daily_review", { reviewDate });
}

export function generateDailyReview(reviewDate: string): Promise<DailyReview> {
  return invoke<DailyReview>("generate_daily_review", { reviewDate });
}
