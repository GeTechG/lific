import { describe, expect, test } from "bun:test";
import { isRunning, logTime, runLogAppended, RUNNING_WINDOW_MS } from "../src/lib/issues/runLog";

describe("run log", () => {
  test("server timestamps are read as UTC", () => {
    expect(logTime("2026-10-02 08:30:00")).toBe(Date.UTC(2026, 9, 2, 8, 30, 0));
  });

  test("an issue is running while its newest line is younger than two minutes", () => {
    const at = logTime("2026-10-02 08:30:00");
    expect(isRunning("2026-10-02 08:30:00", undefined, at + RUNNING_WINDOW_MS - 1)).toBe(true);
    expect(isRunning("2026-10-02 08:30:00", undefined, at + RUNNING_WINDOW_MS)).toBe(false);
    expect(isRunning(undefined, undefined, at)).toBe(false);
    expect(isRunning(null, at, at + 1000)).toBe(true);
    // A line seen live outranks the stale timestamp the row was read with.
    expect(isRunning("2026-10-02 08:00:00", at, at + 1000)).toBe(true);
    expect(isRunning("2026-10-02 08:30:00", at - 10 * 60_000, at + 1000)).toBe(true);
  });

  test("only a run_log.appended event with its ids under `log` is a log event", () => {
    const log = { project_id: 1, issue_id: 7, last_id: 42 };
    expect(runLogAppended({ type: "run_log.appended", log })).toEqual(log);
    expect(runLogAppended({ type: "issue.updated", issue_id: 7, project_id: 1 })).toBeNull();
    expect(runLogAppended({ type: "run_log.appended" })).toBeNull();
    expect(runLogAppended({ type: "run_log.appended", log: { issue_id: "7" } })).toBeNull();
  });
});
