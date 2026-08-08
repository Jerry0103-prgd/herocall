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
  createdAt: string;
};

export type DeepSeekStatus = { status: "已配置" | "未配置" };

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
  return invoke<AiReview>("generate_ai_review", { reviewDate });
}
