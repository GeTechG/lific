import type { BrowserContext, Page } from "playwright";

/** Call the API as the signed-in user of `page`. */
function api(page: Page, method: string, path: string, body?: unknown) {
  return page.evaluate(
    async ({ method, path, body }) => {
      const res = await fetch(`/api${path}`, {
        method,
        headers: {
          Authorization: `Bearer ${localStorage.getItem("lific_token")}`,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      if (!res.ok) throw new Error(`${method} ${path}: ${res.status} ${await res.text()}`);
      return res.json();
    },
    { method, path, body },
  );
}

/**
 * The run log is hidden on an issue without lines, appears when lines are
 * appended over HTTP while the page is open (no reload), keeps appending,
 * leaves the issue's seq alone, and marks the issue as running on the list
 * and the board.
 */
export async function checkIssueRunLog(context: BrowserContext, base: string) {
  const page = await context.newPage();
  const writer = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(String(err)));
  try {
    await writer.goto(`${base}/DEMO/issues/DEMO-1`);
    const projects = await api(writer, "GET", "/projects");
    const project = projects.find((p: { identifier: string }) => p.identifier === "DEMO");
    const issue = await api(writer, "POST", "/issues", {
      project_id: project.id,
      title: "Running smoke issue",
      status: "active",
    });

    await page.goto(`${base}/DEMO/issues/${issue.identifier}`);
    await page.getByTestId("issue-assignee").waitFor({ state: "visible" });
    const section = page.getByTestId("issue-run-log");
    if ((await section.count()) !== 0) throw new Error("run log shown for an issue with no lines");

    // Appended from another tab while this one stays open.
    await api(writer, "POST", `/issues/${issue.id}/log`, {
      source: "run-7",
      lines: ["cloning the repository", "running the tests"],
    });
    await section.getByText("running the tests").waitFor({ state: "visible", timeout: 10_000 });
    await section.locator("[data-log-source]").filter({ hasText: "run-7" }).waitFor();
    await api(writer, "POST", `/issues/${issue.id}/log`, { source: "run-7", lines: ["pushing the branch"] });
    await section.getByText("pushing the branch").waitFor({ state: "visible", timeout: 10_000 });
    if ((await section.locator("[data-log-line]").count()) !== 3) {
      throw new Error("expected exactly three log lines, each once");
    }
    await section.locator('[data-running="true"]').waitFor({ state: "visible" });
    console.log("ok   run log appears and appends live, without a reload");

    const after = await api(writer, "GET", `/issues/${issue.id}`);
    if (after.seq !== issue.seq) throw new Error(`log lines moved seq ${issue.seq} -> ${after.seq}`);
    console.log("ok   run log lines leave the issue's seq alone");

    await page.goto(`${base}/DEMO/issues`);
    const row = page.getByRole("group", { name: issue.identifier, exact: true });
    await row.locator('[data-running="true"]').waitFor({ state: "visible", timeout: 15_000 });
    await page.goto(`${base}/DEMO/board`);
    const card = page.locator("article").filter({ hasText: "Running smoke issue" });
    await card.locator('[data-running="true"]').waitFor({ state: "visible", timeout: 15_000 });
    const idle = page.locator("article").filter({ hasText: "Second smoke issue" });
    await idle.waitFor({ state: "visible" });
    if ((await idle.locator('[data-running="true"]').count()) !== 0) {
      throw new Error("an issue with no log lines is marked running");
    }
    console.log("ok   list row and board card mark the running issue");

    if (errors.length > 0) throw new Error(`page errors: ${errors.join("; ")}`);
  } finally {
    await writer.close();
    await page.close();
  }
}
