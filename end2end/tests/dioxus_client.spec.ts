import { expect, test } from "@playwright/test";

/**
 * Dioxus wasm client checks (MIGRATION.md Phase 5 slice 5e). Run against a
 * second gate server (port 8131) whose public dir is the `dx build` output,
 * so the SSR pages boot the wasm bundle: the router must take over
 * navigation, and the links editor's reactive submit path (`use_action`
 * against the `POST /api/update_links` twin) must replace the no-JS 303
 * flow. `run-dioxus-gate.sh` passes `DIOXUS_URL` for this server. The
 * bundle is a debug build, so wasm boot gets generous timeouts.
 */
const base = process.env.DIOXUS_URL ?? "http://127.0.0.1:8131";

/** The page's level-1 heading. */
const h1 = (page: import("@playwright/test").Page) => page.getByRole("heading", { level: 1 });

/**
 * Waits until the hydrated client owns the page.
 *
 * The SSR markup carries one `data-dioxus-id` (the `#main` root the
 * interpreter initialises); every listener node the client hydrates gets
 * its own id, so a handful of them means hydration has finished and clicks
 * are handled by the router rather than the browser.
 */
const clientIsLive = async (page: import("@playwright/test").Page) => {
  await page.waitForFunction(() => document.querySelectorAll("[data-dioxus-id]").length > 5, undefined, {
    timeout: 30000,
  });
};

/** Log in through the login form; success 303s home with the user menu. */
const loginAs = async (page: import("@playwright/test").Page, username: string) => {
  await page.goto(`${base}/login`);
  await page.fill('input[name="username"]', username);
  await page.fill('input[name="password"]', "password");
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".navbar summary")).toContainText(username);
};

/** Same admin gate as the server-side spec (e2e-admin is seeded out-of-band). */
const requireAdmin = () =>
  test.skip(process.env.E2E_ADMIN_OK !== "1", "needs e2e-admin seeded; see run-dioxus-gate.sh");

test("the bundle boots clean and the router takes over navigation", async ({ page }) => {
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  await page.goto(`${base}/admin/artist/e2e-tabs`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Profile");
  await clientIsLive(page);

  // With the client live, each tab click is an SPA navigation: the URL
  // changes and no document request goes out.
  const loadsBeforeTabs = documentLoads;
  await page.getByRole("tab", { name: "Releases" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/releases\/?$/, { timeout: 15000 });
  await expect(h1(page)).toHaveText("E2E Tabs Artist Releases", { timeout: 15000 });
  expect(documentLoads, "the Releases click should be SPA navigation").toBe(loadsBeforeTabs);

  // The client's route display for the nest index carries an empty query
  // string, so the SPA URL ends in `/` or `/?`.
  await page.getByRole("tab", { name: "Profile" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs(?:\/|\/\?)?$/, { timeout: 15000 });
  expect(documentLoads, "the Profile click should be SPA navigation").toBe(loadsBeforeTabs);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the links editor submits reactively (no 303 round trip)", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  const stamp = Date.now();
  const firstUrl = `https://e2e.example.com/artist/${stamp}`;
  await page.goto(`${base}/admin/artist/e2e-tabs/links`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Links");
  await clientIsLive(page);

  // The no-JS fallback is still wired (the form targets the 303 handler),
  // but with the client live the submit is intercepted.
  await expect(page.locator('form[action="/admin/forms/links"]')).toHaveCount(1);

  await page.fill('input[name="spotify"]', firstUrl);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.getByText("E2E Tabs Artist Links Updated!")).toBeVisible({ timeout: 15000 });
  await expect(page).not.toHaveURL(/updated=1/);
  await expect(page.locator('input[name="spotify"]')).toHaveValue(firstUrl);

  // Second submit keeps the URL steady too, and the value comes from the
  // action's result rather than a reload.
  const secondUrl = `https://e2e.example.com/artist/${stamp}-reactive`;
  await page.fill('input[name="spotify"]', secondUrl);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.locator('input[name="spotify"]')).toHaveValue(secondUrl);
  await expect(page).not.toHaveURL(/updated=1/);

  // Reactive clear: the service diffs empty fields against saved rows.
  await page.fill('input[name="spotify"]', "");
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.locator('input[name="spotify"]')).toHaveValue("");
});
