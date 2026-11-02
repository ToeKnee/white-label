import { expect, test } from "@playwright/test";

/**
 * Dioxus shell navigation smoke test (see MIGRATION.md Phases 3-4). Run against
 * a live Dioxus server via `end2end/run-dioxus-gate.sh`; override the target
 * with DIOXUS_URL. The public site (home, artists, artist, release, page) is
 * real since slice 2 and is rendered server-side from the gate's database, so
 * these assertions read the label and artist names from the page instead of
 * hardcoding fixture data. Auth and admin pages are still placeholders until
 * their slices land, so those checks cover the chrome, route resolution and
 * Link hrefs only.
 * There is no wasm bundle yet, so clicks are full page loads (hydration smoke
 * coverage comes once the client targets land, Phases 4-7).
 */
const base = process.env.DIOXUS_URL ?? "http://127.0.0.1:8127";

/** The navbar brand link, which shows the record label's name. */
const brandLink = (page: import("@playwright/test").Page) => page.locator(".navbar-start a");

/** The page's level-1 heading. */
const h1 = (page: import("@playwright/test").Page) => page.getByRole("heading", { level: 1 });

/** The record label name shown in the navbar brand. */
const labelName = async (page: import("@playwright/test").Page) => {
  await expect(brandLink(page)).not.toHaveText("");
  return (await brandLink(page).innerText()).trim();
};

test("home renders the shell: navbar, main, footer", async ({ page }) => {
  await page.goto(`${base}/`);
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.getByRole("link", { name: "Login" })).toBeVisible();

  // The label name drives the brand, the h1 and the <title> (the last Title
  // on the page wins, so the page's own title replaces the app-level one).
  const label = await labelName(page);
  await expect(h1(page)).toHaveText(label);
  await expect(page).toHaveTitle(label);

  await expect(page.locator("footer").getByText("Services")).toBeVisible();
});

test("navbar and footer links navigate", async ({ page }) => {
  await page.goto(`${base}/`);
  await page.getByRole("link", { name: "Register" }).click();
  await expect(h1(page)).toHaveText("Register");

  await page.getByRole("link", { name: "Privacy policy" }).click();
  await expect(page).toHaveURL(/\/pages\/privacy-policy$/);
  // The gate database has no `privacy-policy` row, so the page still renders
  // the shell while the data endpoint answers 404 and the h1 stays empty.
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();

  // Back home via the brand link, whose text is the label name.
  await brandLink(page).click();
  await expect(h1(page)).toHaveText(await labelName(page));
});

test("the artists index lists the public artists", async ({ page }) => {
  await page.goto(`${base}/`);
  const label = await labelName(page);

  await page.goto(`${base}/artists`);
  await expect(h1(page)).toHaveText(`${label} Artists`);
  // Artist cards are whole-card links with the artist name as the card title.
  await expect(page.locator("a .card-title").first()).toBeVisible();
});

test("an unknown artist slug renders an empty artist page", async ({ page }) => {
  // A missing artist is logged and rendered as an empty artist page rather
  // than redirected, as the Leptos original did; the 404 comes from the data
  // endpoint, not from the route.
  await page.goto(`${base}/artists/there-is-no-such-artist`);
  await expect(page).toHaveURL(/\/artists\/[a-z-]+$/);
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});

test("an unknown release route keeps the shell and shows 404", async ({ page }) => {
  // Deviation from the Leptos originals, which answered a missing artist or
  // release under `/artists/...` with a 303 redirect. Dioxus cannot re-render
  // an ancestor route out of a nested one while streaming SSR, so the unmatched
  // segment falls through to the app's 404 page inside the shell. The Rust SSR
  // tests in `crate::server` cover the in-process redirect attempts; revisit
  // once the client router lands (Phases 6-7).
  await page.goto(`${base}/`);
  await labelName(page);
  await page.goto(`${base}/artists/there-is-no-such-artist/releases/nonsense`);
  await expect(h1(page)).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});

test("an unknown release slug under a known artist shows 404", async ({ page }) => {
  // Deviation from the Leptos original, which redirected to the artist page:
  // dioxus cannot re-render an ancestor route out of a nested one during SSR,
  // so the deeper segment stays unmatched and the app's NotFound page renders.
  // See the open note in MIGRATION.md (Phase 4 slice 2). The artist slug comes
  // from the artists index so this needs no fixture names.
  await page.goto(`${base}/artists`);
  const firstCard = page
    .locator("a")
    .filter({ has: page.locator(".card-title") })
    .first();
  const artistHref = await firstCard.getAttribute("href");
  expect(artistHref).toMatch(/^\/artists\/[a-z0-9-]+$/);

  await page.goto(`${base}${artistHref}/releases/there-is-no-such-release`);
  await expect(h1(page)).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});

test("nested admin tabs navigate and mark the active tab", async ({ page }) => {
  await page.goto(`${base}/admin/artist/jimi`);
  await expect(h1(page)).toHaveText("Edit artist");
  // The tabs are `role="tab"` links rendered by the AdminArtistRoot layout.
  await expect(page.getByRole("tab", { name: "Profile" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Releases" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/jimi\/releases\/?$/);
  await expect(h1(page)).toHaveText("Releases");
  await expect(page.getByRole("tab", { name: "Releases" })).toHaveClass(/tab-active/);
});

test("unknown paths show the 404 page inside the shell", async ({ page }) => {
  await page.goto(`${base}/totally/bogus/path`);
  await expect(h1(page)).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});
