import { expect, test } from "@playwright/test";

/**
 * Dioxus shell navigation smoke test (see MIGRATION.md Phases 3-4). Run against
 * a live Dioxus server via `end2end/run-dioxus-gate.sh`; override the target
 * with DIOXUS_URL. The public site (home, artists, artist, release, page),
 * the auth pages (login/register/logout/profile) and the admin section
 * (gate, dashboard, label/page/artist-profile/release/track/links editors)
 * are real since slices 2-5d and are rendered server-side from the gate's
 * database, so these assertions read
 * the label and artist names from the page instead of hardcoding fixture
 * data. The gate seeds an `e2e-dioxus`/`password` account through the app's
 * own register handler (idempotent across runs); admin tests need an
 * `e2e-admin` account seeded with the admin/label_owner permissions
 * out-of-band and are skipped unless the gate reports `E2E_ADMIN_OK`.
 * The artist images tab is real since Phase 5 slice 5g; uploading needs the
 * wasm client, so this spec only pins its SSR markup (the client spec uploads
 * for real).
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

/** Log in through the login form; success 303s home with the user menu. */
const loginAs = async (page: import("@playwright/test").Page, username: string) => {
  await page.goto(`${base}/login`);
  await page.fill('input[name="username"]', username);
  await page.fill('input[name="password"]', "password");
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".navbar summary")).toContainText(username);
};

/**
 * Gate for the admin-browser tests. `e2e-admin` (admin + label_owner) can
 * only be seeded out-of-band — the app's register handler grants no
 * permissions — so `run-dioxus-gate.sh` exports `E2E_ADMIN_OK=1` when the
 * account works; without it these tests skip (the Rust SSR tests in
 * `crate::server` cover the same flows in-process).
 */
const requireAdmin = () =>
  test.skip(process.env.E2E_ADMIN_OK !== "1", "needs e2e-admin seeded; see run-dioxus-gate.sh");

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

test("login rejects wrong credentials and shows a fixed alert", async ({ page }) => {
  await page.goto(`${base}/login`);
  await page.fill('input[name="username"]', "e2e-dioxus");
  await page.fill('input[name="password"]', "definitely-wrong");
  await page.getByRole("button", { name: "Login" }).click();
  // The form posts to /auth/login; the handler 303s back with a fixed code,
  // and the page renders the matching alert (the code itself is never shown).
  await expect(page).toHaveURL(/\/login\?bad=password$/);
  await expect(page.getByText("Password does not match.")).toBeVisible();
});

test("the signed-in flow: user menu, profile form and logout", async ({ page }) => {
  await page.goto(`${base}/login`);
  await page.fill('input[name="username"]', "e2e-dioxus");
  await page.fill('input[name="password"]', "password");
  await page.getByRole("button", { name: "Login" }).click();
  // Success 303s to the home page and the navbar switches to the user menu.
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".navbar summary")).toContainText("e2e-dioxus");

  await page.goto(`${base}/profile`);
  await expect(h1(page)).toHaveText("Edit Profile");
  await expect(page.locator('input[name="username"]')).toHaveValue("e2e-dioxus");
  await expect(page.getByRole("button", { name: "Update" })).toBeVisible();

  // Logout: open the navbar dropdown, visit the logout page, then press its
  // button (the form posts and 303s back with ?done=1).
  await page.locator(".navbar summary").click();
  await page.getByRole("link", { name: "Log out" }).click();
  await page.getByRole("button", { name: "Log out" }).click();
  await expect(page).toHaveURL(/\/logout\?done=1$/);
  await expect(page.getByText("Logged out")).toBeVisible();
  await expect(page.getByRole("link", { name: "Login" })).toBeVisible();
});

test("a guest visiting a guarded page is served the login page", async ({ page }) => {
  // Deviation note (MIGRATION.md slice 3): a component-level redirect renders
  // the login page *inside* the current URL (dioxus has no 303 from SSR
  // components); the login form is what the guest gets to interact with.
  await page.goto(`${base}/profile`);
  await expect(h1(page)).toHaveText("Login");
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
});

test("a guest visiting an admin page is served the login page too", async ({ page }) => {
  // The slice-4 `AdminRoot` gate uses the same pattern; the admin menu
  // never renders for guests.
  await page.goto(`${base}/admin`);
  await expect(h1(page)).toHaveText("Login");
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
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
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // `e2e-tabs` is seeded idempotently by end2end/run-dioxus-gate.sh; the
  // profile editor (slice 5a) fetches the real row, so the header is the
  // artist name + "Profile" rather than the old placeholder.
  await page.goto(`${base}/admin/artist/e2e-tabs`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Profile");
  // The tabs are `role="tab"` links rendered by the AdminArtistRoot layout.
  await expect(page.getByRole("tab", { name: "Profile" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Releases" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/releases\/?$/);
  // Slice 5b made the releases tab real: the list header names the artist.
  await expect(h1(page)).toHaveText("E2E Tabs Artist Releases");
  await expect(page.getByRole("tab", { name: "Releases" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Links" }).click();
  // (Links to routes with optional query segments render a trailing `?`.)
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/links\??$/);
  // Slice 5d made the links tab real: the editor header names the artist.
  await expect(h1(page)).toHaveText("E2E Tabs Artist Links");
  await expect(page.getByRole("tab", { name: "Links" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Images" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/images\/?$/);
  // Slice 5g made the images tab real: header + the upload picker (the upload
  // itself is the client's job, asserted in dioxus_client.spec.ts).
  await expect(h1(page)).toHaveText("E2E Tabs Artist Images");
  await expect(page.getByRole("tab", { name: "Images" })).toHaveClass(/tab-active/);
  await expect(page.locator('input[name="file_to_upload"]')).toHaveCount(1);
});

test("the admin dashboard renders cards and the side menu", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // The navbar exposes the Admin link only for `admin` sessions.
  await page.locator(".navbar summary").click();
  await page.getByRole("link", { name: "Admin" }).click();
  await expect(page).toHaveURL(/\/admin\/?$/);
  await expect(page).toHaveTitle("Admin Dashboard");

  // Side menu (served with URLs built by the admin_menu service) and the
  // dashboard card row.
  await expect(page.locator("ul.menu").first()).toBeVisible();
  await expect(page.getByRole("link", { name: "Dashboard" })).toBeVisible();
  const label = await labelName(page);
  await expect(page.getByRole("heading", { level: 2, name: "Artists" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Pages" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Social Tips!" })).toBeVisible();
  // The label card names the label (label_owner session).
  await expect(page.getByText(label).first()).toBeVisible();
});

test("page create, update, delete and restore round-trip", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // Create through the dashboard's Add button and the create form; the
  // server 303s to the new page's editor under the slug it derived.
  const stamp = Date.now();
  await page.goto(`${base}/admin/page`);
  await page.fill('input[name="name"]', `e2e Page ${stamp}`);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/page/e2e-page-${stamp}$`));
  await expect(h1(page)).toHaveText(`Edit e2e Page ${stamp}`);

  // Update with a rename: `update` re-slugifies, and the 303 lands on the
  // new slug carrying ?updated=1 plus the success alert.
  await page.fill('input[name="name"]', `e2e Renamed ${stamp}`);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/page/e2e-renamed-${stamp}\\?updated=1$`));
  await expect(page.getByText(`e2e Renamed ${stamp} Updated!`)).toBeVisible();

  // Delete/Restore are JS-free form buttons (no dialog): the row's Delete
  // posts, and the reload swaps in Restore.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page).toHaveURL(/updated=1$/);
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();

  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();

  // Leave the fixture soft-deleted so reruns don't pile up live pages.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();
});

test("artist profile create, rename, delete and restore round-trip", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // Create posts to /admin/forms/artist; the 303 lands on the profile
  // editor (inside the tab layout) under the slug derived from the name.
  const stamp = Date.now();
  await page.goto(`${base}/admin/artist`);
  await expect(h1(page)).toHaveText("Create Artist");
  await page.fill('input[name="name"]', `e2e Artist ${stamp}`);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-artist-${stamp}$`));
  await expect(h1(page)).toHaveText(`e2e Artist ${stamp} Profile`);
  await expect(page.getByRole("tab", { name: "Profile" })).toBeVisible();

  // Update with a rename: the artist update re-slugifies too, and the 303
  // lands on the new slug carrying ?updated=1 plus the success alert.
  await page.fill('input[name="name"]', `e2e Renamed Artist ${stamp}`);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-renamed-artist-${stamp}\\?updated=1$`));
  await expect(page.getByText(`e2e Renamed Artist ${stamp} Updated!`)).toBeVisible();

  // Delete/Restore are the same JS-free form-association buttons as the
  // page editor: the reload swaps Delete for Restore on the row.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();

  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();

  // Leave the fixture soft-deleted so reruns don't pile up live artists.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();
});

test("release create, rename, delete and restore round-trip", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // Create through the releases list Add button; the 303 lands silently on
  // the release editor under the slug derived from the name (the create
  // handler carries no ?updated=1, parity with the Leptos action).
  const stamp = Date.now();
  await page.goto(`${base}/admin/artist/e2e-tabs/releases`);
  await page.getByRole("link", { name: "Add" }).click();
  await expect(h1(page)).toHaveText("New Release");
  await page.fill('input[name="name"]', `e2e Release ${stamp}`);
  await page.fill('input[name="catalogue_number"]', `E2E-CAT-${stamp}`);
  // The primary-artist select and the artist checkbox start on the
  // addressed artist; leave both defaults.
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-tabs/release/e2e-release-${stamp}$`));
  // Pin the heading by name (robust against extra headings on the page).
  await expect(page.getByRole("heading", { level: 1, name: `e2e Release ${stamp} - E2E Tabs Artist` })).toBeVisible();

  // Update with a rename: `update` re-slugifies, and the 303 lands on the
  // new slug carrying ?updated=1 plus the success alert.
  await page.fill('input[name="name"]', `e2e Renamed ${stamp}`);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-tabs/release/e2e-renamed-${stamp}\\?updated=1$`));
  await expect(page.getByText(`e2e Renamed ${stamp} Updated!`)).toBeVisible();

  // Delete/Restore use the same JS-free form-association buttons; the
  // reload swaps Delete for Restore on the row.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();

  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();

  // Leave the fixture soft-deleted so reruns don't pile up live releases.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();
});

test("track create, rename, delete and restore round-trip", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // Create a fresh release first (the track pages are addressed through one).
  const stamp = Date.now();
  await page.goto(`${base}/admin/artist/e2e-tabs/releases`);
  await page.getByRole("link", { name: "Add" }).click();
  await expect(h1(page)).toHaveText("New Release");
  await page.fill('input[name="name"]', `e2e Track Release ${stamp}`);
  await page.fill('input[name="catalogue_number"]', `E2E-TRK-CAT-${stamp}`);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-tabs/release/e2e-track-release-${stamp}$`));

  // The release editor's Tracks sidebar (real since slice 5c) links to the
  // track create page through its Add button.
  await page.getByRole("link", { name: "Add" }).click();
  // (Links to routes with optional query segments render a trailing `?`.)
  await expect(page).toHaveURL(new RegExp(`/admin/artist/e2e-tabs/release/e2e-track-release-${stamp}/tracks/new\\??$`));
  await expect(h1(page)).toHaveText("New Track");
  await page.fill('input[name="name"]', `e2e Track ${stamp}`);
  await page.fill('input[name="track_number"]', "1");
  // ISRC must be 12 characters and is unique across reruns; bpm is
  // optional. The artist pickers start on the addressed artist.
  await page.fill('input[name="isrc_code"]', `E2EX26${String(stamp).slice(-6)}`);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page).toHaveURL(
    new RegExp(`/admin/artist/e2e-tabs/release/e2e-track-release-${stamp}/track/e2e-track-${stamp}$`),
  );
  // The editor header is "Track - Release - Artist".
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: `e2e Track ${stamp} - e2e Track Release ${stamp} - E2E Tabs Artist`,
    }),
  ).toBeVisible();

  // Update with a rename: `update` re-slugifies, and the 303 lands on the
  // new slug carrying ?updated=1 plus the success alert.
  await page.fill('input[name="name"]', `e2e Renamed Track ${stamp}`);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page).toHaveURL(
    new RegExp(
      `/admin/artist/e2e-tabs/release/e2e-track-release-${stamp}/track/e2e-renamed-track-${stamp}\\?updated=1$`,
    ),
  );
  await expect(page.getByText(`e2e Renamed Track ${stamp} Updated!`)).toBeVisible();

  // Delete/Restore use the JS-free form-association buttons; the reload
  // swaps Delete for Restore on the row.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();

  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();

  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();

  // Leave both fixtures soft-deleted so reruns don't pile up live rows.
  await page.goto(`${base}/admin/artist/e2e-tabs/release/e2e-track-release-${stamp}`);
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible();
});

test("artist links save and clear through the 303 fallback", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // The links editor (slice 5d) renders one input per platform with the
  // saved URL pre-filled. There is no wasm client yet, so Update is a
  // full-page POST that 303s back with ?updated=1 and the alert.
  const stamp = Date.now();
  const spotifyUrl = `https://e2e.example.com/artist/${stamp}`;
  await page.goto(`${base}/admin/artist/e2e-tabs/links`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Links");

  await page.fill('input[name="spotify"]', spotifyUrl);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/links\?updated=1$/);
  await expect(page.getByText("E2E Tabs Artist Links Updated!")).toBeVisible();
  await expect(page.locator('input[name="spotify"]')).toHaveValue(spotifyUrl);

  // Clearing the input deletes the row (the service diffs empty fields
  // against the saved links), and the reload renders an empty input.
  await page.fill('input[name="spotify"]', "");
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.locator('input[name="spotify"]')).toHaveValue("");
});

test("unknown paths show the 404 page inside the shell", async ({ page }) => {
  await page.goto(`${base}/totally/bogus/path`);
  await expect(h1(page)).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});
