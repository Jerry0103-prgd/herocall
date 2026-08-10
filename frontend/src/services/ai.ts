import { invoke } from "@tauri-apps/api/core";

export type AiServiceStatus = {
  configured: boolean;
  model: string | null;
};

export type AiReview = {
  id: number;
  reviewId: number;
  model: string;
  promptVersion: string;
  facts: string[];
  inferences: string[];
  risks: string[];
  report: AiResearchReport | null;
  securityId: number | null;
  securityName: string | null;
  securitySymbol: string | null;
  createdAt: string;
};

export type AiResearchReport = {
  stockStatus: string;
  marketAnalysis: string;
  sectorAnalysis: string;
  newsAnalysis: string;
  technicalAnalysis: string;
  strategyReference: string;
  conclusion: string;
};

export type DeepSeekStatus = { status: "已配置" | "未配置" };

export type AiProviderConfig = {
  provider: "DEEPSEEK" | "TENCENT_TOKENHUB" | "DOUBAO";
  displayName: string;
  model: string;
  configured: boolean;
  enabled: boolean;
  isCurrent: boolean;
  priority: number;
};

export type AiProviderConnectionTest = {
  provider: AiProviderConfig["provider"];
  model: string;
  success: boolean;
  httpStatus: number | null;
  message: string;
};

export function loadDeepSeekStatus(): Promise<DeepSeekStatus> {
  return invoke<DeepSeekStatus>("get_deepseek_status");
}

export function saveDeepSeekApiKey(key: string): Promise<DeepSeekStatus> {
  return invoke<DeepSeekStatus>("save_deepseek_api_key", { key });
}

export function removeDeepSeekApiKey(): Promise<DeepSeekStatus> {
  return invoke<DeepSeekStatus>("remove_deepseek_api_key");
}

export function loadAiServiceStatus(): Promise<AiServiceStatus> {
  return invoke<AiServiceStatus>("get_ai_service_status");
}

export function loadLatestAiReview(reviewId: number): Promise<AiReview | null> {
  return invoke<AiReview | null>("get_latest_ai_review", { reviewId });
}

export function generateAiReview(reviewDate: string): Promise<AiReview> {
  return invoke<AiReview>("generate_ai_review_for_snapshot", { reviewDate });
}

export function loadAiProviderConfigs(): Promise<AiProviderConfig[]> {
  return invoke<AiProviderConfig[]>("get_ai_provider_configs");
}

export function saveAiProviderApiKey(provider: AiProviderConfig["provider"], key: string): Promise<DeepSeekStatus> {
  return invoke<DeepSeekStatus>("save_ai_provider_api_key", { provider, key });
}

export function removeAiProviderApiKey(provider: AiProviderConfig["provider"]): Promise<DeepSeekStatus> {
  return invoke<DeepSeekStatus>("remove_ai_provider_api_key", { provider });
}

export function setAiProviderEnabled(provider: AiProviderConfig["provider"], enabled: boolean): Promise<AiProviderConfig[]> {
  return invoke<AiProviderConfig[]>("set_ai_provider_enabled", { provider, enabled });
}

export function testAiProviderConnection(provider: AiProviderConfig["provider"]): Promise<AiProviderConnectionTest> {
  return invoke<AiProviderConnectionTest>("test_ai_provider_connection", { provider });
}

export function generateAiReviews(reviewDate: string): Promise<AiReview[]> {
  return invoke<AiReview[]>("generate_ai_reviews_for_snapshot", { reviewDate });
}

export function loadAiReviewsForDate(reviewDate: string): Promise<AiReview[]> {
  return invoke<AiReview[]>("get_ai_reviews_for_date", { reviewDate });
}
