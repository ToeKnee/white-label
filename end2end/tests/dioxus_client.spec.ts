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

/**
 * Logs in through the login form. Whether that submit is the no-JS 303 or
 * the reactive twin (`/api/client/login`) depends on whether the wasm client
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
  // The upload must swap this without a page load (SessionHint::ProfileUpdated).
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
  // The slice 6d twins (`/api/client/login|logout`) rather than the 303 form
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
  // `<img>` moves onto it without a reload — Leptos kept showing the previous
  // file until the page was reloaded.
  await expect(page.locator(".alert-success")).toContainText(/Uploaded .+-e2e-tabs\.png/, {
    timeout: 30000,
  });
  await expect(shown).toHaveAttribute("src", /\/uploads\/artists\/\d+-e2e-tabs\.png$/);
  expect(await shown.getAttribute("src"), "the shown image should change").not.toBe(before);
  expect(documentLoads, "an upload is a fetch, not a form post").toBe(loadsBeforeUpload);

  expect(pageErrors, `client page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});
