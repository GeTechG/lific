import { expect, test } from "bun:test";
import { STATUSES, buildGroups, isUnresolved } from "../src/lib/issues/grouping";
import type { Issue } from "../src/lib/api";

test("in_review sits between active and done and counts as unresolved", () => {
  expect(STATUSES).toEqual(["backlog", "todo", "active", "in_review", "done", "cancelled"]);
  expect(isUnresolved("in_review")).toBe(true);
  expect(isUnresolved("done")).toBe(false);
});

test("status groups keep the wire value as key and show a readable label", () => {
  const issue = (id: number, status: string) => ({ id, status }) as Issue;
  const groups = buildGroups({
    sortedIssues: [issue(1, "done"), issue(2, "in_review"), issue(3, "active")],
    modules: [],
    groupBy: "status",
    searchQuery: "",
    filterStatus: "",
  });
  expect(groups?.map((group) => [group.key, group.label])).toEqual([
    ["active", "active"],
    ["in_review", "in review"],
    ["done", "done"],
  ]);
});
