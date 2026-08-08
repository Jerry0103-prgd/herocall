import { invoke } from "@tauri-apps/api/core";

export type EventStatus = "CONFIRMED" | "UNCONFIRMED" | "ARCHIVED";

export type CalendarEvent = {
  id: number;
  eventType: "EARNINGS" | "DIVIDEND" | "EX_DIVIDEND" | "SHAREHOLDER_MEETING" | "MACRO_DATA" | "FED_MEETING";
  title: string;
  eventTime: string;
  timezone: string;
  source: string;
  sourceUrl: string | null;
  status: EventStatus;
  relatedSecurity: string | null;
  holdingRelated: boolean;
};

export function loadCalendarEvents(status?: EventStatus): Promise<CalendarEvent[]> {
  return invoke<CalendarEvent[]>("get_calendar_events", { status: status ?? null });
}
