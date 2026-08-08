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

export function loadHoldingNewsArticles(): Promise<NewsArticle[]> {
  return invoke<NewsArticle[]>("get_holding_news_articles");
}
