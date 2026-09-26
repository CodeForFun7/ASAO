import type {
  AsaoSettings,
  WidgetRecommendation,
  WidgetSystemUpdate,
} from "../types/widget";

export interface RecommendationService {
  evaluateRecommendation(
    update: WidgetSystemUpdate,
    settings: AsaoSettings
  ): WidgetRecommendation | null;
}

class DeterministicRecommendationService implements RecommendationService {
  private lastNotifiedId: string | null = null;
  private lastNotifiedTimestamp = 0;

  evaluateRecommendation(
    update: WidgetSystemUpdate,
    settings: AsaoSettings
  ): WidgetRecommendation | null {
    if (!settings.showRecommendations) {
      return null;
    }

    const rec = update.recommendation;

    // Trigger non-intrusive desktop notification only for 'important' priority
    // and rate-limit to at most once every 3 minutes per distinct issue.
    if (
      settings.notifications &&
      rec.priority === "important" &&
      rec.id !== this.lastNotifiedId &&
      Date.now() - this.lastNotifiedTimestamp > 180_000
    ) {
      this.lastNotifiedId = rec.id;
      this.lastNotifiedTimestamp = Date.now();

      if (
        typeof window !== "undefined" &&
        "Notification" in window &&
        Notification.permission === "granted"
      ) {
        new Notification("ASAO System Notice", {
          body: rec.message,
          silent: true,
        });
      }
    }

    return rec;
  }
}

export const recommendationService: RecommendationService =
  new DeterministicRecommendationService();
