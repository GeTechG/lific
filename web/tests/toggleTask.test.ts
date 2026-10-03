import { expect, test } from "bun:test";
import { toggleTask } from "../src/lib/markdownFormat";

const md = ["# Plan", "- [ ] one", "  * [x] nested", "```", "- [ ] in code", "```", "1. [ ] numbered", "> - [X] quoted", "- [ ]not a task"].join("\n");

test("flips the n-th rendered checkbox, skipping fenced code", () => {
  expect(toggleTask(md, 0, true)).toContain("- [x] one");
  expect(toggleTask(md, 1, false)).toContain("  * [ ] nested");
  expect(toggleTask(md, 2, true)).toContain("1. [x] numbered");
  expect(toggleTask(md, 2, true)).toContain("- [ ] in code");
  expect(toggleTask(md, 3, false)).toContain("> - [ ] quoted");
});

test("no such checkbox: the body is returned unchanged", () => {
  expect(toggleTask(md, 4, true)).toBe(md);
  expect(toggleTask("plain text", 0, true)).toBe("plain text");
});
