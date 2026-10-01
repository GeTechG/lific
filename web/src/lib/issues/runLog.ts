// The run log: lines a scheduler appends to an issue while an agent works
// it. Pure helpers; the live state is in runLogLive.svelte.ts.

/** An issue counts as running while its newest line is younger than this. */
export const RUNNING_WINDOW_MS = 2 * 60 * 1000;

export interface RunLogAppended {
  project_id: number;
  issue_id: number;
  last_id: number;
}

/** The payload of a `run_log.appended` realtime event, or null for any
 *  other event. The ids sit under `log`, not at the top level, so views that
 *  refetch on `issue_id` / `project_id` leave it alone. */
export function runLogAppended(event: { type: string; [key: string]: unknown }): RunLogAppended | null {
  if (event.type !== "run_log.appended") return null;
  const log = event.log as Partial<RunLogAppended> | undefined;
  return typeof log?.issue_id === "number" &&
    typeof log.project_id === "number" &&
    typeof log.last_id === "number"
    ? (log as RunLogAppended)
    : null;
}

/** Server timestamps are UTC, written `YYYY-MM-DD HH:MM:SS`. */
export function logTime(ts: string): number {
  return Date.parse(`${ts.replace(" ", "T")}Z`);
}

/** Whether a line was logged within the last two minutes: either the read's
 *  `last_log_at` or a realtime event seen at `liveAt` (local epoch ms). */
export function isRunning(
  lastLogAt: string | null | undefined,
  liveAt: number | undefined,
  nowMs: number,
): boolean {
  const newest = Math.max(lastLogAt ? logTime(lastLogAt) : -Infinity, liveAt ?? -Infinity);
  return nowMs - newest < RUNNING_WINDOW_MS;
}
