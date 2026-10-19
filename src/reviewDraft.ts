import type { Inspection } from './domain';
import type { ExportPreferences } from './preferences';

// Committed review data only. Browser access and operation state belong to
// native export recovery, never to an ordinary review draft.
export interface ReviewDraft {
  schemaVersion: 1;
  sessionId: string;
  revision: number;
  url: string;
  inspection: Inspection;
  selected: number[];
  preferences: ExportPreferences;
}

export function makeReviewDraft(
  sessionId: string,
  revision: number,
  inspection: Inspection,
  selected: ReadonlySet<number>,
  preferences: ExportPreferences,
): ReviewDraft {
  return {
    schemaVersion: 1,
    sessionId,
    revision,
    url: inspection.sourceUrl,
    inspection,
    selected: [...selected].sort((a, b) => a - b),
    preferences,
  };
}
