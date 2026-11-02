import { expect, test } from "@playwright/test";

/**
 * Phase 3 shell navigation smoke test (see MIGRATION.md). Run against a live
 * Dioxus server via `end2end/run-dioxus-gate.sh`; override the target with
 * DIOXUS_URL. Pages are placeholders until the Phase 4 slices, so this
 * checks the chrome, route resolution and Link hrefs — not real content.
 * There is no wasm bundle yet, so clicks are full page loads (hydration
 * smoke coverage comes once the client targets land, Phases 4-7).
 */
const base = process.env.DIOXUS_URL ?? "http://127.0.0.1:8127";

test("home renders the shell: navbar, main, footer", async ({ page }) => {
  await page.goto(`${base}/`);
  await expect(page).toHaveTitle("White Label");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.getByRole("link", { name: "Login" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Record label home",
  );
  await expect(page.locator("footer").getByText("Services")).toBeVisible();
});

test("navbar and footer links navigate", async ({ page }) => {
  await page.goto(`${base}/`);
  await page.getByRole("link", { name: "Register" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Register");

  await page.getByRole("link", { name: "Privacy policy" }).click();
  await expect(page).toHaveURL(/\/pages\/privacy-policy$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Page");
  // The chrome is identical on every page.
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();

  // Back home via the brand link (label name still empty at this phase, so
  // select it positionally).
  await page.locator(".navbar-start a").click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Record label home",
  );
});

test("nested admin tabs navigate and mark the active tab", async ({ page }) => {
  await page.goto(`${base}/admin/artist/jimi`);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Edit artist",
  );
  // The tabs are `role="tab"` links rendered by the AdminArtistRoot layout.
  await expect(page.getByRole("tab", { name: "Profile" })).toHaveClass(
    /tab-active/,
  );

  await page.getByRole("tab", { name: "Releases" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/jimi\/releases\/?$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Releases");
  await expect(page.getByRole("tab", { name: "Releases" })).toHaveClass(
    /tab-active/,
  );
});

test("unknown paths show the 404 page inside the shell", async ({ page }) => {
  await page.goto(`${base}/totally/bogus/path`);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});
