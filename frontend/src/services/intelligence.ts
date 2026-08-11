import { invoke } from "@tauri-apps/api/core";

export type IntelligenceItem = {
  id: number;
  title: string;
  summary: string;
  source: string;
  sourceType: "OFFICIAL" | "NEWS" | "INDUSTRY" | "COMMUNITY" | "SOCIAL" | "RUMOR";
  sourceUrl: string | null;
  publishedAt: string;
  credibilityLevel: "A" | "B" | "C" | "D" | "E";
  intelligenceType: string;
  topicKey: string;
  importanceScore: number;
  status: "ACTIVE" | "UNVERIFIED" | "PARTIALLY_CONFIRMED";
};

export type IntelligenceTopic = {
  title: string;
  sourceCount: number;
  sourceTypes: string[];
  credibilityLevels: string[];
};

export type SecurityIntelligence = {
  securityId: number;
  securityName: string;
  securitySymbol: string;
  discussionHeat: "NO_DATA" | "LOW" | "NORMAL" | "HIGH" | "SURGING";
  sentiment: string;
  topics: IntelligenceTopic[];
  importantItems: IntelligenceItem[];
  communityOpinions: string[];
  rumors: IntelligenceItem[];
  summary: string;
};

export type MarketIntelligence = {
  securities: SecurityIntelligence[];
  partialUnavailableSources: string[];
};

export type RadarEvent = {
  id: number;
  eventType: string;
  title: string;
  eventTime: string;
  timezone: string;
  source: string;
  sourceUrl: string | null;
  relatedSecurity: string | null;
  credibilityLevel: "A" | "B" | "C" | "D" | "E";
  potentialImpact: "POSITIVE" | "NEUTRAL" | "NEGATIVE" | "UNCERTAIN";
};

export type MarketRadar = {
  next24Hours: RadarEvent[];
  next3Days: RadarEvent[];
  next7Days: RadarEvent[];
};

export function loadMarketIntelligence(): Promise<MarketIntelligence> {
  return invoke<MarketIntelligence>("get_market_intelligence");
}

export function loadMarketRadar(): Promise<MarketRadar> {
  return invoke<MarketRadar>("get_market_radar");
}
