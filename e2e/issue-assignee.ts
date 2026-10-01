import type { BrowserContext } from "playwright";

/**
 * Assign an issue from its sidebar picker, find the indicator on the detail
 * view, the issue list and the board, then clear it again.
 */
export async function checkIssueAssignee(context: BrowserContext, base: string, username: string) {
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(String(err)));
  try {
    await page.goto(`${base}/DEMO/issues/DEMO-1`);
    const identifier = await page.evaluate(async () => {
      const headers = {
        Authorization: `Bearer ${localStorage.getItem("lific_token")}`,
        "Content-Type": "application/json",
      };
      const projects = await (await fetch("/api/projects", { headers })).json();
      const project = projects.find((p: { identifier: string }) => p.identifier === "DEMO");
      const res = await fetch("/api/issues", {
        method: "POST",
        headers,
        body: JSON.stringify({ project_id: project.id, title: "Assigned smoke issue", status: "todo" }),
      });
      if (!res.ok) throw new Error(`create: ${res.status} ${await res.text()}`);
      return (await res.json()).identifier as string;
    });

    await page.goto(`${base}/DEMO/issues/${identifier}`);
    const field = page.getByTestId("issue-assignee");
    const chip = `[data-assignee="${username}"]`;
    await field.getByRole("button", { name: "Unassigned" }).click();
    // The menu is fixed-position, outside the field — and must still open
    // under its trigger: the sidebar's translate makes the aside the menu's
    // containing block, which once pushed it past the right edge.
    const option = page.getByRole("button", { name: "Smoke Operator", exact: true });
    const box = await option.boundingBox();
    const width = page.viewportSize()?.width ?? 0;
    if (!box || box.x < 0 || box.x + box.width > width) {
      throw new Error(`assignee menu opened off-screen: ${JSON.stringify(box)} in a ${width}px viewport`);
    }
    await option.click();
    await field.locator(chip).waitFor({ state: "visible" });
    console.log("ok   issue detail assigns from the member picker");

    await page.goto(`${base}/DEMO/issues`);
    const row = page.getByRole("group", { name: identifier, exact: true });
    await row.locator(chip).waitFor({ state: "visible", timeout: 15_000 });
    await page.goto(`${base}/DEMO/board`);
    const card = page.locator("article").filter({ hasText: "Assigned smoke issue" });
    await card.locator(chip).waitFor({ state: "visible", timeout: 15_000 });
    console.log("ok   list row and board card show the assignee");

    await page.goto(`${base}/DEMO/issues/${identifier}`);
    await field.getByRole("button", { name: "Smoke Operator" }).click();
    await page.getByRole("button", { name: "Unassigned", exact: true }).click();
    await field.locator(chip).waitFor({ state: "detached" });
    await page.reload();
    await field.getByRole("button", { name: "Unassigned" }).waitFor({ state: "visible" });
    console.log("ok   issue detail clears the assignee");

    if (errors.length > 0) throw new Error(`page errors: ${errors.join("; ")}`);
  } finally {
    await page.close();
  }
}
