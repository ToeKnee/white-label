import { expect, test } from "@playwright/test";

/**
 * Dioxus wasm client checks.
 * Run against a second gate server (port 8131) whose public dir is the
 * `dx build` output, so the SSR pages boot the wasm bundle: the router must
 * take over navigation, and the editors' reactive submit path (`use_action`
 * against the JSON twins under `/api/*`) must replace the no-JS 303
 * flow without a document load. `run-dioxus-gate.sh` passes `DIOXUS_URL` for
 * this server. The bundle is a debug build, so wasm boot gets generous
 * timeouts.
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

/**
 * Logs in through the login form. Whether that submit is the no-JS 303 or
 * the reactive twin (`/api/login`) depends on whether the wasm client
 * has hydrated yet — either way the visitor ends up home with the user menu
 * in the navbar, which is what these tests need before they browse.
 */
const loginAs = async (page: import("@playwright/test").Page, username: string) => {
  await page.goto(`${base}/login`);
  await page.fill('input[name="username"]', username);
  await page.fill('input[name="password"]', "password");
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".navbar summary")).toContainText(username);
};

/**
 * A real 1x1 PNG (`file` reports: PNG image data, 1x1, 8-bit/color RGBA, 70
 * bytes). The server never decodes images, so any bytes would stream, but an
 * actual png keeps the declared mime type honest.
 */
const PNG_1X1_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/**
 * The smallest real WAV: a 44-byte `RIFF`/`WAVE` header and no samples. The
 * audio upload test posts this, so both the browser's declared type and the
 * server's signature check see a real wave container.
 */
const WAV_BYTES = (() => {
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36, 4);
  header.write("WAVEfmt ", 8);
  header.writeUInt32LE(16, 16); // the `fmt ` chunk's size
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(8000, 24); // 8kHz
  header.writeUInt32LE(16000, 28); // byte rate
  header.writeUInt16LE(2, 32); // block align
  header.writeUInt16LE(16, 34); // 16-bit
  header.write("data", 36);
  header.writeUInt32LE(0, 40);
  return header;
})();

/** A progress bar sighting: what the DOM carried at the time it was seen. */
type Sighting = { max: string | null; value: string | null };

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

  // The footer's label column heading hydrates through its own boundary.
  await expect(page.locator("footer .footer-title").nth(1)).not.toHaveText("", {
    timeout: 15000,
  });

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

test("the release editor creates, saves, deletes and restores reactively", async ({ page }) => {
  // The `/api/*_release` twins. A desktop window has no page
  // load to carry `?updated=1` or `?bad=<code>`, so the twins answer the saved
  // row (or the same code) as data and the editor renders from that. The web
  // client runs the very same path, so no document request may go out.
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  // A unique name and catalogue number keep the run self-contained (the
  // catalogue number has to be unique within the label, and the gate prunes
  // `e2e-reactive-%` rows before each run).
  const stamp = Date.now();
  const name = `E2E Reactive ${stamp}`;
  const renamed = `E2E Reactive ${stamp} Renamed`;

  await page.goto(`${base}/admin/artist/e2e-tabs/releases/new`);
  await expect(h1(page)).toHaveText("New Release");
  await clientIsLive(page);
  const loadsBefore = documentLoads;

  // The no-JS fallback is still wired, but with the client live the submit is
  // intercepted, and the new row's editor is reached by SPA navigation (what
  // the 303 handler does with its Location).
  await expect(page.locator('form[action="/admin/forms/release"]')).toHaveCount(1);
  await page.fill('input[name="name"]', name);
  await page.fill('input[name="catalogue_number"]', `E2E-${stamp}`);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(h1(page)).toHaveText(`${name} - E2E Tabs Artist`, { timeout: 15000 });
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/release\/e2e-reactive-\d+/, {
    timeout: 15000,
  });

  // The side menu's listing follows the create. Its load resolves once per
  // mount and the layout outlives every admin navigation, so the accepted
  // save bumps the counter the load tracks and the row's link shows up
  // without a document load.
  await expect(page.locator(`ul.w-56 a[href*="e2e-reactive-${stamp}"]`)).not.toHaveCount(0, {
    timeout: 15000,
  });

  // A reactive save renders from the answer instead of a reload: the alert and
  // the header follow the rename, no `?updated=1` appears, and the hidden slug
  // the next save (and the uploader) targets moves with it.
  await page.fill('input[name="name"]', renamed);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.locator(".alert-success")).toContainText(`${renamed} Updated!`, {
    timeout: 15000,
  });
  await expect(page.locator(".alert-error")).toHaveCount(0);
  await expect(h1(page)).toHaveText(`${renamed} - E2E Tabs Artist`);
  await expect(page).not.toHaveURL(/updated=1/);
  // The field form's hidden slug (the page also carries the uploader's and the
  // delete/restore siblings', which follow the same row).
  const slug = await page
    .locator('form[action="/admin/forms/release/update"] input[name="slug"]')
    .getAttribute("value");
  expect(slug, "the hidden slug follows the rename").toMatch(/-renamed$/);

  // The same follow-through on the rename: the menu's link for this release
  // now targets the new slug (an unrefreshed menu would still link the old
  // one, which 404s).
  await expect(page.locator('ul.w-56 a[href*="-renamed"]')).not.toHaveCount(0, {
    timeout: 15000,
  });

  // Soft delete swaps the editor's Delete button for Restore with no page load
  // (the row in the answer carries `deleted_at`), and restore swaps it back.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible({ timeout: 15000 });
  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible({ timeout: 15000 });

  // Leave the fixture soft-deleted: the other specs never need it, and the
  // gate's prune step removes these rows before the next run.
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("button", { name: "Restore" })).toBeVisible({ timeout: 15000 });

  // And on the delete: with "show deleted" off, the refreshed menu load
  // drops the soft-deleted row.
  await expect(page.locator('ul.w-56 a[href*="-renamed"]')).toHaveCount(0, { timeout: 15000 });

  expect(documentLoads, "create, save, delete and restore are all fetches").toBe(loadsBefore);
  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the menu moves between two releases of the same artist", async ({ page }) => {
  // The Ideas.md report: navigating between two rows of the same route
  // variant (release to release) reuses the editor's component instance,
  // and a `use_page` load only re-runs on signal changes, so the editor
  // used to stay pinned to the first row. The body now remounts on a
  // param-derived key, so the second row loads — inside one document.
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  // Create a second release under `e2e-tabs` reactively and land in its
  // editor (the gate prunes `e2e-reactive-%` rows before each run).
  const stamp = Date.now();
  await page.goto(`${base}/admin/artist/e2e-tabs/releases/new`);
  await expect(h1(page)).toHaveText("New Release");
  await clientIsLive(page);
  const loadsBefore = documentLoads;

  await page.fill('input[name="name"]', `E2E Reactive ${stamp}`);
  await page.fill('input[name="catalogue_number"]', `E2E-${stamp}`);
  const menuRefreshed = page.waitForResponse((response) => response.url().includes("/api/admin_menu"), {
    timeout: 15000,
  });
  await page.getByRole("button", { name: "Create" }).click();
  // The accepted create re-fetched the menu, so its listing is settled and
  // both releases' links are in it.
  await menuRefreshed;
  await expect(h1(page)).toHaveText(`E2E Reactive ${stamp} - E2E Tabs Artist`, { timeout: 15000 });

  // Open the artist's submenu (a closed `details` by default) and click
  // the seeded release's link: same route variant, only the params differ.
  await page.locator("ul.w-56 summary a", { hasText: "E2E Tabs Artist" }).evaluate((anchor) => {
    (anchor.closest("details") as HTMLDetailsElement).open = true;
  });
  await page.locator('ul.w-56 a[href*="e2e-audio-release"]').click();

  // The editor has to follow the navigation to the other row. Before the
  // remount-on-param-key fix it stayed on the reactive release.
  await expect(h1(page)).toHaveText("E2E Audio Release - E2E Tabs Artist", { timeout: 15000 });
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/release\/e2e-audio-release/, {
    timeout: 15000,
  });
  expect(documentLoads, "a menu click is SPA navigation").toBe(loadsBefore);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the profile page saves reactively and keeps the session", async ({ page }) => {
  // A window has no page load to carry
  // `/profile?saved=1`, so the twin answers with the stored account, which the
  // form re-fills and the navbar shows. An earlier build signed the window's
  // user out on a profile save, so the chrome is part of what this checks.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-dioxus");
  await page.goto(`${base}/profile`);
  await expect(h1(page)).toHaveText("Edit Profile");
  await clientIsLive(page);
  const loadsBefore = documentLoads;

  const stamp = Date.now();
  const firstName = `E2E ${stamp}`;
  await expect(page.locator('form[action="/auth/profile"]')).toHaveCount(1);
  await page.fill('input[name="first_name"]', firstName);
  await page.getByRole("button", { name: "Update" }).click();
  await expect(page.locator(".alert-success")).toContainText("e2e-dioxus Updated!", {
    timeout: 15000,
  });
  await expect(page.locator(".alert-error")).toHaveCount(0);
  await expect(page).not.toHaveURL(/saved=1/);

  // The form and the chrome both follow the answer the save gave.
  await expect(page.locator('input[name="first_name"]')).toHaveValue(firstName);
  await expect(page.locator(".navbar summary")).toContainText("e2e-dioxus");
  expect(documentLoads, "the save is a fetch, not a form post").toBe(loadsBefore);

  // The twin really wrote the account: a fresh document re-fills the same value
  // from the session.
  await page.goto(`${base}/profile`);
  await expect(page.locator('input[name="first_name"]')).toHaveValue(firstName, {
    timeout: 15000,
  });

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the profile avatar upload runs through the server fns with a progress bar", async ({ page }) => {
  // Unlike the admin specs this one only needs a session: the avatar slot
  // asks for no extra permissions, so the plain `e2e-dioxus` seed user works.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-dioxus");

  const picked = {
    name: "e2e-avatar.png",
    mimeType: "image/png",
    buffer: Buffer.from(PNG_1X1_BASE64, "base64"),
  };

  await page.goto(`${base}/profile`);
  await expect(h1(page)).toHaveText("Edit Profile");
  await clientIsLive(page);

  // The picker markup is SSR'd, and no bar is rendered until an upload runs.
  // That the submit below uploads at all is the proof the client re-attached
  // its listener to the form (and that `FormEvent` really carries the file).
  await expect(page.locator('input[name="file_to_upload"]')).toHaveCount(1);
  await expect(page.locator("progress")).toHaveCount(0);

  // The navbar avatar before the upload (a placeholder or an earlier file).
  // The upload must swap it without a page load: the profile page hands the
  // navbar a fresh SessionHint::SignedIn carrying the updated user.
  const navbarAvatar = page.locator(".navbar .avatar img");
  const beforeAvatarSrc = await navbarAvatar.getAttribute("src");

  // The bar only exists while an upload is in flight, so its sightings are
  // recorded by an observer installed before the click instead of polled.
  await page.evaluate(() => {
    const recorded = window as unknown as { progressSightings: { max: string | null; value: string | null }[] };
    recorded.progressSightings = [];
    const record = () => {
      for (const bar of document.querySelectorAll("progress.w-56")) {
        recorded.progressSightings.push({
          max: bar.getAttribute("max"),
          value: bar.getAttribute("value"),
        });
      }
    };
    new MutationObserver(record).observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["max", "value"],
    });
  });

  const loadsBeforeUpload = documentLoads;
  await page.setInputFiles('input[name="file_to_upload"]', picked);
  await page.getByRole("button", { name: "Upload" }).click();

  // The alert carries the stored name (`<timestamp>-<username>.png`: the
  // avatar config renames the upload to the uploader). The wire tests in
  // `crate::ui::upload` cover the storage itself.
  await expect(page.locator(".alert-success")).toContainText(/Uploaded .+-e2e-dioxus\.png/, {
    timeout: 30000,
  });
  await expect(page.locator(".alert-error")).toHaveCount(0);
  expect(documentLoads, "an upload is a fetch, not a form post").toBe(loadsBeforeUpload);

  // The navbar avatar follows the upload on the same live document: the
  // profile page hands the stored name to the chrome, which points the
  // `<img>` at `/uploads/avatars/<stored>` (the cache-busting timestamp in the
  // stored name is what makes the browser fetch the new bytes).
  const stored = /Uploaded (\S+-e2e-dioxus\.png)/.exec((await page.locator(".alert-success").textContent()) ?? "")?.[1];
  expect(stored, "the alert names the stored file").toBeTruthy();
  await expect(navbarAvatar).toHaveAttribute("src", `/uploads/avatars/${stored}`, {
    timeout: 15000,
  });
  expect(beforeAvatarSrc, "the avatar url should actually change so the browser refetches it").not.toBe(
    `/uploads/avatars/${stored}`,
  );

  const sightings: Sighting[] = await page.evaluate(
    () =>
      (window as unknown as { progressSightings: { max: string | null; value: string | null }[] }).progressSightings,
  );
  const size = String(picked.buffer.length);
  expect(sightings.length, "the bar renders while uploading").toBeGreaterThan(0);
  expect(
    sightings.every((sighting) => sighting.max === size),
    `the bar is sized to the picked file, saw ${JSON.stringify(sightings)}`,
  ).toBe(true);
  // The bar retires when the outcome lands, so the page keeps only the alert.
  await expect(page.locator("progress")).toHaveCount(0);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("auth submits reactively: same document, navbar follows the session", async ({ page }) => {
  // The `/api/login|logout` twins rather than the 303 form
  // handlers. A marker set on the live document is the proof: a form post
  // would load a fresh document and take it with it.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await page.goto(`${base}/login`);
  await expect(h1(page)).toHaveText("Login");
  await clientIsLive(page);
  await page.evaluate(() => {
    (window as unknown as { authMarker?: string }).authMarker = "live document";
  });
  const loadsBefore = documentLoads;

  // A reactive failure: the twin answers with the `password` code and the
  // page's own alert table renders it — no redirect, so no `?bad=` in the URL.
  await page.fill('input[name="username"]', "e2e-dioxus");
  await page.fill('input[name="password"]', "definitely-wrong");
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page.locator(".alert-error")).toContainText("Password does not match.", {
    timeout: 15000,
  });
  await expect(page).not.toHaveURL(/bad=/);
  expect(documentLoads, "a failed login is a fetch, not a form post").toBe(loadsBefore);

  // The right password: the twin answers with the user, and the navbar menu
  // renders that user straight away (the chrome outlives the navigation, so
  // this is how it catches up).
  await page.fill('input[name="password"]', "password");
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page).toHaveURL(/\/$/, { timeout: 15000 });
  await expect(page.locator(".navbar summary")).toContainText("e2e-dioxus", { timeout: 15000 });

  // And back out again: the menu item is an SPA navigation to the logout
  // page, whose button ends the session — then the session hint puts the
  // guest links back without the logout 303 (`?done`) ever running.
  await page.locator(".navbar summary").click();
  await page.getByRole("link", { name: "Log out" }).click();
  await expect(h1(page)).toHaveText("Log out", { timeout: 15000 });
  await page.getByRole("button", { name: "Log out" }).click();
  await expect(page.locator(".navbar").getByRole("link", { name: "Login" })).toBeVisible({
    timeout: 15000,
  });
  await expect(page).not.toHaveURL(/done=/);

  expect(documentLoads, "three submits, no document load").toBe(loadsBefore);
  expect(
    await page.evaluate(() => (window as unknown as { authMarker?: string }).authMarker),
    "the live document should survive all three submits",
  ).toBe("live document");
  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the artist images tab uploads and shows the new image", async ({ page }) => {
  // Unlike the avatar slot, the Artist slot asks for `admin` + `label_owner`,
  // so this run needs the seeded admin account.
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  await page.goto(`${base}/admin/artist/e2e-tabs/images`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Images");
  await clientIsLive(page);

  const shown = page.getByAltText("E2E Tabs Artist");
  const before = await shown.getAttribute("src");

  const picked = {
    name: "e2e-artist.png",
    mimeType: "image/png",
    buffer: Buffer.from(PNG_1X1_BASE64, "base64"),
  };
  const loadsBeforeUpload = documentLoads;
  await page.setInputFiles('input[name="file_to_upload"]', picked);
  await page.getByRole("button", { name: "Upload" }).click();

  // The alert names the stored file (`<timestamp>-<artist slug>.png`) and the
  // `<img>` moves onto it without a reload.
  await expect(page.locator(".alert-success")).toContainText(/Uploaded .+-e2e-tabs\.png/, {
    timeout: 30000,
  });
  await expect(shown).toHaveAttribute("src", /\/uploads\/artists\/\d+-e2e-tabs\.png$/);
  expect(await shown.getAttribute("src"), "the shown image should change").not.toBe(before);
  expect(documentLoads, "an upload is a fetch, not a form post").toBe(loadsBeforeUpload);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the track editor uploads audio and plays it without a reload", async ({ page }) => {
  // The audio slot asks for `admin` + `label_owner`, like the image slots, and
  // its editor is addressed by artist, release and track (the gate seeds
  // `e2e-audio-release`/`e2e-audio-track` under `e2e-tabs`).
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  await page.goto(`${base}/admin/artist/e2e-tabs/release/e2e-audio-release/track/e2e-audio-track`);
  await expect(h1(page)).toHaveText("E2E Audio Track - E2E Audio Release - E2E Tabs Artist");
  await clientIsLive(page);

  const picked = { name: "e2e-audio.wav", mimeType: "audio/wav", buffer: WAV_BYTES };
  const loadsBeforeUpload = documentLoads;
  // The editor now hosts two uploaders, so the audio slot's own form is what
  // gets the file and the click.
  const audioForm = page.locator('form:has(input[name="file_type"][value="TrackAudio"])');
  await page.setInputFiles('input[name="file_to_upload"][accept*=".wav"]', picked);
  await audioForm.getByRole("button", { name: "Upload" }).click();

  // The alert names the stored file (`<timestamp>-<track slug>.wav`: audio is
  // named after the track, like an artist image after the artist) and the
  // player appears on the same live document. `crate::ui::upload`'s wire tests
  // cover the storage itself.
  await expect(page.locator(".alert-success")).toContainText(/Uploaded .+-e2e-audio-track\.wav/, { timeout: 30000 });
  await expect(page.locator('audio[src^="/uploads/tracks/"]')).toHaveCount(1, {
    timeout: 15000,
  });
  expect(documentLoads, "an upload is a fetch, not a form post").toBe(loadsBeforeUpload);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the track editor uploads its cover image without a reload", async ({ page }) => {
  // The image slot on the same editor as the audio test above: its upload
  // lands in the tracks directory (like the audio, told apart by the
  // extension) and swaps into the editor's `<img>` on the live document.
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  await page.goto(`${base}/admin/artist/e2e-tabs/release/e2e-audio-release/track/e2e-audio-track`);
  await expect(h1(page)).toHaveText("E2E Audio Track - E2E Audio Release - E2E Tabs Artist");
  await clientIsLive(page);

  const shown = page.getByAltText("E2E Audio Track");
  const before = await shown.getAttribute("src");

  const picked = {
    name: "e2e-cover.png",
    mimeType: "image/png",
    buffer: Buffer.from(PNG_1X1_BASE64, "base64"),
  };
  const loadsBeforeUpload = documentLoads;
  const imageForm = page.locator('form:has(input[name="file_type"][value="TrackImage"])');
  await page.setInputFiles('input[name="file_to_upload"][accept*=".png"]', picked);
  await imageForm.getByRole("button", { name: "Upload" }).click();

  // The alert names the stored file (`<timestamp>-<track slug>.png`), and only
  // the image widget alerts: a success on one slot must not show on the other.
  await expect(page.locator(".alert-success")).toContainText(/Uploaded .+-e2e-audio-track\.png/, { timeout: 30000 });
  await expect(page.locator(".alert-success")).toHaveCount(1);
  await expect(shown).toHaveAttribute("src", /\/uploads\/tracks\/\d+-e2e-audio-track\.png$/, {
    timeout: 15000,
  });
  expect(await shown.getAttribute("src"), "the shown image should change").not.toBe(before);
  expect(documentLoads, "an upload is a fetch, not a form post").toBe(loadsBeforeUpload);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("Add to Queue grows the live queue without disturbing playback", async ({ page }) => {
  // While the bar plays, the rows offer `+ Add to Queue` instead of `⏵
  // Play`: a click appends its track to the end of the queue (here the
  // playing track itself, repeated twice), and the loaded track — the
  // tagged audio node — is untouched while the bar's place counts up.
  // The queueing listener signs in first: signed-in listening is exempt
  // from the guest play allowance entirely (the allowance is under test
  // elsewhere).
  requireAdmin();
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await loginAs(page, "e2e-admin");
  await page.goto(`${base}/artists/e2e-tabs/e2e-audio-release`);
  await expect(h1(page)).toHaveText("E2E Audio Release");
  await clientIsLive(page);

  await page.getByRole("button", { name: "Play E2E Audio Track" }).click();
  await expect(page.locator("#global-player")).toContainText("1 of 1");
  const add = page.getByRole("button", { name: "Add E2E Audio Track to queue" });
  await expect(add).toHaveClass(/btn-active/, "the loaded row's button changed");
  await page.locator("#global-player audio").evaluate((el) => ((el as HTMLElement).dataset.wlProbe = "kept"));

  await add.click();
  // The fixture's zero-sample wave ends (so the bar advances) as soon as it
  // loads, hence these pins watch the queue *total* — only Add to Queue can
  // raise it — rather than the ordinal.
  await expect(page.locator("#global-player")).toContainText("of 2");
  await add.click();
  await expect(page.locator("#global-player")).toContainText("of 3");

  // The bar itself stayed put: one media element on the page, and the
  // tagged node was never rebuilt.
  await expect(page.locator("audio")).toHaveCount(1, "only the bar makes sound");
  await expect(page.locator('#global-player audio[data-wl-probe="kept"]')).toHaveCount(1);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the global player keeps its track through SPA navigation", async ({ page }) => {
  // `e2e-audio-track`'s audio name can change mid-run (the upload specs post
  // to the same fixture row), so pins assert the `/uploads/tracks/` shape
  // rather than a fixed file name.
  // The Ideas.md ask: keep listening while moving around the site. The bar
  // is chrome outside the `Outlet`, so navigation swaps only the page body;
  // the bar's `<audio>` node — tagged below with a dataset — must survive it
  // rather than being rebuilt. A guest needs no session to listen.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await page.goto(`${base}/artists/e2e-tabs/e2e-audio-release`);
  await expect(h1(page)).toHaveText("E2E Audio Release");
  await clientIsLive(page);

  // With the bar closed the row offers `⏵ Play`, and no media is on the
  // page yet (the bar is the only player).
  await expect(page.locator("audio")).toHaveCount(0, "the media only lives in the bar");
  await expect(page.locator("#global-player")).toHaveCount(0);
  await page.getByRole("button", { name: "Play E2E Audio Track" }).click();

  // The bar took over: track and place show, and it is the only media on
  // the page; the row's button became `+ Add to Queue` — the bar plays,
  // and the guest's browser still has plays of the track to spare.
  await expect(page.locator("#global-player")).toBeVisible();
  await expect(page.locator("#global-player")).toContainText("E2E Audio Track");
  await expect(page.locator("#global-player")).toContainText("1 of 1");
  await expect(page.locator("audio")).toHaveCount(1, "only the bar makes sound");
  await expect(page.locator("#global-player audio")).toHaveAttribute("src", /^\/uploads\/tracks\//);
  // The bar always shows artwork: this fixture row's own cover (an earlier
  // run's upload may have left one) or the logo, since `e2e-audio-release`
  // has no album art of its own.
  await expect(page.locator("#global-player img")).toHaveCount(1);
  await expect(page.locator("#global-player img")).toHaveAttribute("src", /(\/uploads\/|Logo\.svg)/);
  await expect(page.getByRole("button", { name: "Play E2E Audio Track" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Add E2E Audio Track to queue" })).toBeVisible();

  // Tag the bar's audio node, then prove *that node* (not just "an audio")
  // survives two SPA navigations: home via the brand link, then the artist
  // page from home's artist list.
  await page.locator("#global-player audio").evaluate((el) => ((el as HTMLElement).dataset.wlProbe = "kept"));
  const loadsBeforeNavigating = documentLoads;

  await page.locator(".navbar-start a").click();
  await expect(page).toHaveURL(`${base}/`);
  await expect(page.locator('#global-player audio[data-wl-probe="kept"]')).toHaveCount(1);
  await expect(page.locator("#global-player")).toContainText("E2E Audio Track");

  await page.locator('a[href="/artists/e2e-tabs"]').click();
  await expect(page).toHaveURL(`${base}/artists/e2e-tabs`);
  await expect(page.locator('#global-player audio[data-wl-probe="kept"]')).toHaveCount(1);
  expect(documentLoads, "the bar outlived SPA navigation, not a reload").toBe(loadsBeforeNavigating);

  // Transport: a one-track queue holds at both ends (previous/next do not
  // wrap) and leaves the tagged node alone.
  await page.getByRole("button", { name: "Next track" }).click();
  await page.getByRole("button", { name: "Previous track" }).click();
  await expect(page.locator("#global-player")).toContainText("1 of 1");
  await expect(page.locator('#global-player audio[data-wl-probe="kept"]')).toHaveCount(1);

  // Close puts the bar away; navigating back to the release (still inside
  // the same document) finds the row offering `⏵ Play` again — the label
  // allows five plays per track, and this browser has spent one — with the
  // row's note naming what is left. The rows never carry media of their
  // own.
  await page.getByRole("button", { name: "Close the player" }).click();
  await expect(page.locator("#global-player")).toHaveCount(0);
  await expect(page.locator("audio")).toHaveCount(0, "the artist page has no track rows");

  await page.locator('a[href="/artists/e2e-tabs/e2e-audio-release"]').click();
  await expect(h1(page)).toHaveText("E2E Audio Release");
  await expect(page.getByRole("button", { name: "Play E2E Audio Track" })).toBeVisible();
  await expect(page.getByRole("button", { name: "No plays left for E2E Audio Track" })).toHaveCount(0);
  await expect(page.getByText("4 plays left", { exact: true })).toBeVisible();
  await expect(page.locator("audio")).toHaveCount(0);
  await expect(page.locator("#global-player")).toHaveCount(0);
  expect(documentLoads, "the whole walk was one document").toBe(loadsBeforeNavigating);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the queue auto-appends the artist's other tracks when it runs out", async ({ page }) => {
  // The Ideas.md ask, on the `e2e-radio` fixtures: album A has one audio
  // track, album B two, and the fetch answers at random, so the pins watch
  // queue *totals*, artist membership and the bar's release line — never
  // ordinals or batch order. Chromium ends a zero-sample fixture the moment
  // it loads, so the bar may already have appended (and run to the end)
  // before the first click; the assertions below hold at every point of
  // that race.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto(`${base}/artists/e2e-radio/e2e-radio-album-a`);
  await expect(h1(page)).toHaveText("E2E Radio Album A");
  await clientIsLive(page);

  await page.getByRole("button", { name: "Play Radio Track A1" }).click();
  await expect(page.locator("#global-player")).toBeVisible();
  await expect(page.locator("#global-player")).toContainText(/Radio Track [AB][12]/);

  // `⏭` past the end of the seeded one-track queue: the artist's other
  // audio tracks join the queue (all three total), and the loaded track now
  // comes from album B — the release name the service carried along.
  await page.getByRole("button", { name: "Next track" }).click();
  await expect(page.locator("#global-player")).toContainText("of 3");
  await expect(page.locator("#global-player")).toContainText(/Radio Track B[12]/);
  await expect(page.locator("#global-player")).toContainText("E2E Radio Album B");

  // The bar's artwork, once a fetched B track is loaded: the radio tracks
  // have no image of their own, so the entry fell back to the album art the
  // continuation service carried along (the gate seeds both radio releases'
  // `primary_image` and their bytes).
  await expect(page.locator("#global-player img")).toHaveAttribute(
    "src",
    /\/uploads\/releases\/999999992-e2e-radio-album-b\.jpg$/,
  );

  // Another `⏭` reaches the end of the fetched batch.
  await page.getByRole("button", { name: "Next track" }).click();
  await expect(page.locator("#global-player")).toContainText("3 of 3");

  // A third `⏭` asks once more and the artist has nothing left outside the
  // queue: the bar sits on the track it reached, and keeps sitting there.
  const title = page.locator("#global-player div.font-semibold");
  const loaded = await title.innerText();
  await page.getByRole("button", { name: "Next track" }).click();
  await expect(title).toHaveText(loaded);
  await expect(page.locator("#global-player")).toContainText("3 of 3");
  await expect(page.locator("audio")).toHaveCount(1, "still only the bar's media");

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("a guest gets a label-sized play allowance per track, remembered across reloads", async ({ page }) => {
  // The Ideas.md ask, on the `e2e-radio` fixtures (never mutated by the
  // upload specs): the gate's label carries the default five-play
  // allowance, so each Play spends one of a track's five for this
  // browser — the row's note counts down — and the sixth load finds the
  // disabled `No plays left`. The counts live in localStorage, which is
  // what survives the reload. Clearing the browser's storage hands the
  // plays back — the rule is "a little harder", not impossible.
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto(`${base}/artists/e2e-radio/e2e-radio-album-a`);
  await expect(h1(page)).toHaveText("E2E Radio Album A");
  await clientIsLive(page);

  // A server-shipped row says nothing about plays: the counts belong to
  // the browser, and this one has spent none yet.
  await expect(page.getByText("plays left")).toHaveCount(0, "a fresh slate says nothing");

  // The first Play spends one of five: the button becomes the bar's
  // `+ Add to Queue`, and the row's note counts down.
  await page.getByRole("button", { name: "Play Radio Track A1" }).click();
  await expect(page.locator("#global-player")).toBeVisible();
  await expect(page.getByRole("button", { name: "Add Radio Track A1 to queue" })).toBeVisible();
  await expect(page.getByText("4 plays left", { exact: true })).toBeVisible();

  // The counts reach localStorage, which is what makes the reload below
  // mean something.
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem("white-label-heard-tracks")))
    .toContain("e2e-radio-track-a1");

  // A reload keeps the counts (the bar starts closed, so the row offers
  // Play again — note and all: the rule belongs to the browser, not to
  // one page render).
  await page.reload();
  await expect(h1(page)).toHaveText("E2E Radio Album A");
  await clientIsLive(page);
  await expect(page.getByRole("button", { name: "Play Radio Track A1" })).toBeVisible();
  await expect(page.getByText("4 plays left", { exact: true })).toBeVisible();

  // Four more Play/Close pairs spend the rest of the allowance.
  for (let spent = 2; spent <= 5; spent += 1) {
    await page.getByRole("button", { name: "Play Radio Track A1" }).click();
    await expect(page.locator("#global-player")).toBeVisible();
    await page.getByRole("button", { name: "Close the player" }).click();
    await expect(page.locator("#global-player")).toHaveCount(0);
  }
  await expect(page.getByRole("button", { name: "Play Radio Track A1" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "No plays left for Radio Track A1" })).toBeVisible();

  // Clearing the browser's storage is the way around it, by design: the
  // Play button returns, and the row stops counting down.
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(h1(page)).toHaveText("E2E Radio Album A");
  await clientIsLive(page);
  await expect(page.getByRole("button", { name: "Play Radio Track A1" })).toBeVisible();
  await expect(page.getByRole("button", { name: "No plays left for Radio Track A1" })).toHaveCount(0);
  await expect(page.getByText("plays left")).toHaveCount(0, "a fresh slate says nothing");

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("a guest play reaches the server's counters; the owner's is not counted", async ({ page }) => {
  // The Ideas.md ask, on the `e2e-radio` fixtures: the bar reports each
  // play it stops to the `/api/track_play` twin, and the stats pages read
  // the counters back. The reports here come from closing the bar (the
  // fixture files are byte stubs, never decodable media — nothing plays,
  // nothing has ever, so the measured play is always a skip; the
  // complete/partial thresholds ride on `classify_play`'s unit tests and
  // the endpoint's wire tests). Only lower-bound arithmetic at the end:
  // the same spec runs in parallel on two browsers against these fixtures,
  // so absolute counters are racy (the guest allowance itself is the
  // previous spec's subject, kept in localStorage rather than the DB).
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  // The counters as the stats pages read them: one row of the release
  // scope's answer, three columns summed. The track editor deliberately no
  // longer carries them. The read goes through an in-page `fetch`, not
  // `page.request`: the session cookie is `Secure` (the tower-sessions
  // default), which the browsers send to http://127.0.0.1 (a secure
  // context) but Playwright's out-of-browser request API will not, so only
  // the page itself can answer as the signed-in owner.
  const totals = async (): Promise<number> => {
    const { status, body } = (await page.evaluate(async () => {
      const response = await fetch("/api/get_play_stats?scope=release&slug=e2e-radio-album-a&range=all&sort=complete");
      return {
        status: response.status,
        body: response.status === 200 ? await response.json() : undefined,
      };
    })) as {
      status: number;
      body?: {
        rows?: { track_slug: string; complete_plays: number; partial_plays: number; skips: number }[];
      };
    };
    expect(status, "the owner should reach the stats twin").toBe(200);
    const row = body?.rows?.find((entry) => entry.track_slug === "e2e-radio-track-a1");
    return row ? row.complete_plays + row.partial_plays + row.skips : 0;
  };
  // Press Close with the report response already being awaited: closing a
  // play this bar never measured (a byte-stub file never loads) must post
  // a skip report, and the answer is what the Ideas list's two rules look
  // like over the wire.
  const closeAndAnswer = async () => {
    const report = page.waitForResponse((response) => response.url().includes("/api/track_play"), {
      timeout: 30000,
    });
    await page.getByRole("button", { name: "Close the player" }).click();
    await expect(page.locator("#global-player")).toHaveCount(0);
    const response = await report;
    expect(response.status(), "the play report endpoint").toBe(200);
    return response.json();
  };

  // Baseline, read by the owner…
  await loginAs(page, "e2e-admin");
  const before = await totals();

  // …who is also the owner's-exclusion case: the owner plays and closes on
  // the release page, and the endpoint answers counted:false (their
  // previewing of their own catalogue is deliberately kept out).
  await page.goto(`${base}/artists/e2e-radio/e2e-radio-album-a`);
  await clientIsLive(page);
  await page.getByRole("button", { name: "Play Radio Track A1" }).click();
  await expect(page.locator("#global-player")).toContainText("E2E Radio Album A");
  expect(await closeAndAnswer(), "the owner's play is not counted").toEqual({ counted: false });

  // Sign out (the reactive logout twin), and replay the same Play/Close
  // pair as a guest: the same close report now counts.
  await page.locator(".navbar summary").click();
  await page.getByRole("link", { name: "Log out" }).click();
  await expect(h1(page)).toHaveText("Log out", { timeout: 15000 });
  await page.getByRole("button", { name: "Log out" }).click();
  await expect(page.locator(".navbar").getByRole("link", { name: "Login" })).toBeVisible({
    timeout: 15000,
  });

  await page.goto(`${base}/artists/e2e-radio/e2e-radio-album-a`);
  await clientIsLive(page);
  await page.getByRole("button", { name: "Play Radio Track A1" }).click();
  await expect(page.locator("#global-player")).toContainText("E2E Radio Album A");
  expect(await closeAndAnswer(), "the guest's play is counted").toEqual({ counted: true });

  // Back into the stats data as the owner: the counters cleared the baseline
  // by at least the one guest report (parallel specs may add more).
  await loginAs(page, "e2e-admin");
  await expect
    .poll(totals, {
      message: `the counters should clear the baseline of ${before} by at least one`,
      timeout: 15000,
    })
    .toBeGreaterThan(before);

  // The poll proves the release scope's series is non-empty (this very run
  // counted a play), so the line chart draws its three lines — skip,
  // partial and complete — and at least one day of points. The y scale
  // draws its gridlines (light) and tick labels (dimmed) with them.
  await page.goto(`${base}/admin/artist/e2e-radio/release/e2e-radio-album-a/stats`);
  await expect(page.locator("polyline[class*='stroke-stats-']")).toHaveCount(3);
  await expect(page.locator("circle[class*='fill-stats-']")).not.toHaveCount(0);
  await expect(page.locator("line[class*='opacity-15']")).not.toHaveCount(0);
  await expect(page.locator("svg text[class*='opacity-60']")).not.toHaveCount(0);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("stats tabs list every track and follow range links without a page load", async ({ page }) => {
  // The Ideas.md ask, on the `e2e-radio` fixtures: the artist tab bar gains
  // a Stats tab; its page lists all three fixture tracks (zero-play rows
  // included), and the window links are SPA navigations — same document,
  // same table. The release-level page lists only its own album's track,
  // and a table row links to that track's editor. Structural pins only:
  // the guest-report spec above moves these fixtures' counters in parallel,
  // so counters and the chart's lines are never asserted.
  requireAdmin();

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await loginAs(page, "e2e-admin");

  await page.goto(`${base}/admin/artist/e2e-radio`);
  await clientIsLive(page);

  // The Stats tab is one of the artist tab bar, and clicking it does not
  // reload the document.
  const loadsBeforeTab = documentLoads;
  await page.getByRole("tab", { name: "Stats" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-radio\/stats/, { timeout: 15000 });
  await expect(h1(page)).toHaveText("E2E Radio Artist Play statistics", { timeout: 15000 });
  expect(documentLoads, "the Stats tab should be SPA navigation").toBe(loadsBeforeTab);

  // All three radio fixture rows are listed (a track nobody heard still
  // shows), each linking to its editor. Scoped to the table: the admin
  // menu carries links to the same slugs.
  const table = page.locator("table");
  await expect(table.getByRole("link", { name: "Radio Track A1" })).toBeVisible();
  await expect(table.getByRole("link", { name: "Radio Track B1" })).toBeVisible();
  await expect(table.getByRole("link", { name: "Radio Track B2" })).toBeVisible();

  // Every row also names its artist and release (the two extra columns).
  await expect(table.getByText("E2E Radio Artist").first()).toBeVisible();
  await expect(table.getByText("E2E Radio Album A").first()).toBeVisible();
  await expect(table.getByText("E2E Radio Album B").first()).toBeVisible();

  // A window link: SPA navigation, and the table follows the new page.
  const loadsBeforeRange = documentLoads;
  await page.getByRole("tab", { name: "30 days" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-radio\/stats\?range=30d/, { timeout: 15000 });
  await expect(table.getByRole("link", { name: "Radio Track A1" })).toBeVisible();
  expect(documentLoads, "switching the window should be SPA navigation").toBe(loadsBeforeRange);

  // A column header: also SPA navigation, and it keeps the window the page
  // is already showing. The counters themselves are racy across the gate's
  // parallel runs, so the pin is the marked column, not the row order.
  const loadsBeforeSort = documentLoads;
  await page.getByRole("link", { name: "Skips" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-radio\/stats\?range=30d&sort=skips/, {
    timeout: 15000,
  });
  await expect(table.getByRole("link", { name: "Radio Track A1" })).toBeVisible();
  await expect(page.getByRole("columnheader", { name: /Skips/ })).toHaveAttribute("aria-sort", "descending");
  expect(documentLoads, "re-sorting the table should be SPA navigation").toBe(loadsBeforeSort);

  // A row's link opens that track's editor, also without a document load.
  await table.getByRole("link", { name: "Radio Track A1" }).click();
  await expect(h1(page)).toHaveText("Radio Track A1 - E2E Radio Album A - E2E Radio Artist", {
    timeout: 15000,
  });
  expect(documentLoads, "a stats row link should be SPA navigation too").toBe(loadsBeforeSort);

  // The release-level page lists only its own album's track.
  await page.goto(`${base}/admin/artist/e2e-radio/release/e2e-radio-album-a/stats`);
  await expect(h1(page)).toHaveText("E2E Radio Album A Play statistics");
  await expect(page.locator("table").getByRole("link", { name: "Radio Track A1" })).toBeVisible();
  await expect(page.locator("table").getByRole("link", { name: "Radio Track B1" })).toHaveCount(0);

  // The label-level page shows the radio rows (artist and release columns
  // included) and no longer links back to the label editor: the admin side
  // menu already carries that link on every admin page.
  await page.goto(`${base}/admin/label/stats`);
  await expect(h1(page)).toHaveText("Play statistics");
  await expect(page.locator("table").getByRole("link", { name: "Radio Track A1" })).toBeVisible();
  await expect(table.getByText("E2E Radio Album A").first()).toBeVisible();
  await expect(page.getByRole("link", { name: "Back to the label editor" })).toHaveCount(0);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("the markdown editor's preview renders live while typing", async ({ page }) => {
  // The reported gap: on Firefox the preview never updated as the admin typed
  // (it only showed the server-saved text). The `MarkdownField` preview now
  // tracks a reactive draft, so it re-renders `markdown_html` per keystroke in
  // every browser. Run in chromium and firefox by the gate.
  requireAdmin();
  await loginAs(page, "e2e-admin");
  await page.goto(`${base}/admin/page`);
  await expect(h1(page)).toHaveText("Create Page");
  await clientIsLive(page);

  const body = page.locator('textarea[name="body"]');
  // The preview is the `div.textarea` (the editor itself is a <textarea> tag,
  // so this selector matches only the preview).
  const preview = page.locator("div.textarea");

  // A fresh create starts with an empty preview.
  await expect(preview).toBeEmpty();

  // Typing renders markdown live, rather than echoing the raw source.
  await body.fill("# Heading\n\nSome **bold** text.");
  await expect(preview.locator("h1")).toHaveText("Heading", { timeout: 10000 });
  await expect(preview.locator("strong")).toHaveText("bold");
  await expect(preview).not.toContainText("**bold**");

  // Clearing it live empties the preview again (the draft, not the saved text).
  await body.fill("");
  await expect(preview).toBeEmpty();
});

test("the housekeeping preview runs through the reactive twin", async ({ page }) => {
  // The reactive Housekeeping card: javascript on, the preview submit is
  // intercepted and answered from `/api/purge_deleted` (no 303 document
  // load). The gate seeds one page past the 90 day window, and the
  // answer's listing names it.
  requireAdmin();
  await loginAs(page, "e2e-admin");
  await page.goto(`${base}/admin`);
  await expect(page.getByRole("heading", { level: 2, name: "Housekeeping" })).toBeVisible();
  await clientIsLive(page);

  let documentLoads = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") documentLoads += 1;
  });

  await page.getByRole("button", { name: "Clean up" }).click();
  await expect(page.getByText("E2E Purge Page")).toBeVisible({ timeout: 10000 });

  // The listing also names the upload files the purge would take with the
  // rows (the gate seeds an aged release/track pair whose files sit under
  // UPLOAD_PATH; see run-dioxus-gate.sh), and the file the sweep finds
  // because no row names it at all.
  await expect(page.getByRole("heading", { level: 3, name: "Upload files" })).toBeVisible({ timeout: 10000 });
  await expect(page.getByText("releases/999999999-e2e-purge-release.jpg")).toBeVisible();
  await expect(page.getByText("tracks/999999999-e2e-purge-track.wav")).toBeVisible();
  await expect(page.getByText("artists/999999998-e2e-orphan.png")).toBeVisible();

  // The confirm button joins the card with the preview's count, and the
  // reactive path never left the document. The gate environment always
  // has files (the seed pair plus the orphan fixture), so the label must
  // count them: "<n> items and <m> files for good".
  await expect(page.getByRole("button", { name: /and \d+ files for good$/ })).toBeVisible();
  expect(documentLoads, "the preview should not reload the document").toBe(0);
});
