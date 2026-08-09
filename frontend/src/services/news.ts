import { invoke } from "@tauri-apps/api/core";

export type NewsSourceType = "OFFICIAL" | "MEDIA" | "COMMUNITY";

export type NewsArticle = {
  id: number;
  title: string;
  source: string;
  sourceType: NewsSourceType;
  publishedAt: string;
  fetchTime: string;
  summary: string;
  url: string;
  relatedSecurity: string | null;
};

export type HoldingNewsResult = {
  articles: NewsArticle[];
  noDataReason: string | null;
};

export function loadHoldingNewsArticles(): Promise<HoldingNewsResult> {
  return invoke<HoldingNewsResult>("get_holding_news_articles");
}
