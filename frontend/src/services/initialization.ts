import { invoke } from "@tauri-apps/api/core";

export type InitializationStatus = {
  completed: boolean;
};

export function loadInitializationStatus(): Promise<InitializationStatus> {
  return invoke<InitializationStatus>("get_initialization_status");
}

export function completeInitialization(): Promise<InitializationStatus> {
  return invoke<InitializationStatus>("complete_initialization");
}
