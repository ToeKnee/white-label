import { expect, test } from "@playwright/test";

/**
 * Dioxus shell navigation smoke test. Run against
 * a live Dioxus server via `end2end/run-dioxus-gate.sh`; override the target
 * with DIOXUS_URL. The public site (home, artists, artist, release, page),
 * the auth pages (login/register/logout/profile) and the admin section
 * (gate, dashboard, label/page/artist-profile/release/track/links editors)
 * are real forms rendered server-side from the gate's
 * database, so these assertions read
 * the label and artist names from the page instead of hardcoding fixture
 * data. The gate seeds an `e2e-dioxus`/`password` account through the app's
 * own register handler (idempotent across runs); admin tests need an
 * `e2e-admin` account seeded with the admin/label_owner permissions
 * out-of-band and are skipped unless the gate reports `E2E_ADMIN_OK`. The
 * upload widget also renders a plain multipart form, so this spec (which runs
 * against the server with no wasm bundle) uploads for real the way a
 * javascript-off browser does; the client spec covers the streaming path.
 * There is no wasm bundle on this server, so clicks are full page loads
 * (hydration smoke coverage is `dioxus_client.spec.ts`).
 */
const base = process.env.DIOXUS_URL ?? "http://127.0.0.1:8127";

/** The smallest real PNG, used by the javascript-off upload test. */
const PNG_1X1_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/**
 * The smallest real WAV: a 44-byte `RIFF`/`WAVE` header and no samples. The
 * audio upload tests post this, so the handler's signature check sees a real
 * wave container rather than a renamed text file.
 */
const emptyWav = () => {
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
};

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

/**
 * Measures what a brand-logo `<img>` is painted *on*. The artwork's ink is fixed
 * (the Spotify wordmark is near-black; Apple and Threads ship no fill at all, so
 * they render black), so readability is a property of the backing: the first
 * opaque background at or above the img, falling back to the white canvas the
 * browser paints. Colours go through a 1x1 canvas because daisyUI themes compute
 * to oklch/lab; `luma` is perceived brightness on a 0..255 scale and `backing`
 * keeps the sRGB channels, so a test can tell a tint from white. `pageLuma` is
 * the `<html>` background, which flips with the colour scheme and so proves an
 * `emulateMedia` call actually changed the theme.
 */
const measureLogoBacking = async (page: import("@playwright/test").Page, logo: string) => {
  const measured = await page.evaluate((selector) => {
    const channels = (colour: string) => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 1;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("canvas 2d is unavailable");
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = colour;
      context.fillRect(0, 0, 1, 1);
      return [...context.getImageData(0, 0, 1, 1).data] as [number, number, number, number];
    };
    const luma = ([red, green, blue]: [number, number, number, number]) =>
      Math.round(0.2126 * red + 0.7152 * green + 0.0722 * blue);

    const img = document.querySelector(selector);
    if (!img) return null;
    let backing: [number, number, number, number] = [255, 255, 255, 255];
    for (let node: Element | null = img; node; node = node.parentElement) {
      const colour = channels(getComputedStyle(node).backgroundColor);
      if (colour[3] === 255) {
        backing = colour;
        break;
      }
    }
    const card = img.closest(".card");
    return {
      backing: [...backing],
      backingLuma: luma(backing),
      cardLuma: card ? luma(channels(getComputedStyle(card).backgroundColor)) : null,
      pageLuma: luma(channels(getComputedStyle(document.documentElement).backgroundColor)),
    };
  }, logo);

  expect(measured, `${logo} should render a logo`).not.toBeNull();
  return measured!;
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
  // The footer's label column heads with the same label name (the boundary
  // inlines it during SSR, like the brand above).
  await expect(page.locator("footer .footer-title").nth(1)).toHaveText(label);
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
  // A component-level redirect renders
  // the login page *inside* the current URL (dioxus has no 303 from SSR
  // components); the login form is what the guest gets to interact with.
  await page.goto(`${base}/profile`);
  await expect(h1(page)).toHaveText("Login");
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
});

test("a guest visiting an admin page is served the login page too", async ({ page }) => {
  // The `AdminRoot` gate uses the same pattern; the admin menu
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

test("the release page offers Play buttons and no media without a client", async ({ page }) => {
  // The global player needs the wasm client, and this server carries no
  // wasm bundle: the SSR page (the same markup the client hydrates) lists
  // the audio track with its `⏵ Play` button, but all media lives in the
  // bar — so there are no players on the page, and a Play click has no
  // client to answer it.
  await page.goto(`${base}/artists/e2e-tabs/e2e-audio-release`);
  await expect(h1(page)).toHaveText("E2E Audio Release");
  await expect(page.getByRole("button", { name: "Play E2E Audio Track" })).toBeVisible();
  await expect(page.locator("audio")).toHaveCount(0, "the media only lives in the bar");
  await expect(page.locator("#global-player")).toHaveCount(0);

  await page.getByRole("button", { name: "Play E2E Audio Track" }).click();
  await expect(page.locator("#global-player")).toHaveCount(0, "no client, so no bar");
  await expect(page.locator("audio")).toHaveCount(0, "and no media appears either");
});

test("an unknown artist slug renders an empty artist page", async ({ page }) => {
  // A missing artist is logged and rendered as an empty artist page rather
  // than redirected; the 404 comes from the data endpoint, not from the route.
  await page.goto(`${base}/artists/there-is-no-such-artist`);
  await expect(page).toHaveURL(/\/artists\/[a-z-]+$/);
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});

test("an unknown release route keeps the shell and shows 404", async ({ page }) => {
  // A missing artist or release under `/artists/...` is not answered with a
  // 303 redirect: dioxus cannot re-render an ancestor route out of a nested
  // one while streaming SSR, so the unmatched segment falls through to the
  // app's 404 page inside the shell. The Rust SSR tests in `crate::server`
  // cover the in-process redirect attempts.
  await page.goto(`${base}/`);
  await labelName(page);
  await page.goto(`${base}/artists/there-is-no-such-artist/releases/nonsense`);
  await expect(h1(page)).toHaveText("Oh No...");
  await expect(page.locator(".navbar")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
});

test("an unknown release slug under a known artist shows 404", async ({ page }) => {
  // Dioxus cannot re-render an ancestor route out of a nested one during SSR,
  // so the deeper segment stays unmatched and the app's NotFound page renders.
  // The artist slug comes
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

test("streaming logos keep a light backing for their fixed dark ink", async ({ page }) => {
  // Official brand artwork cannot be recoloured (the Spotify wordmark is
  // near-black, Apple ships no fill at all), while the `black` theme paints the
  // page itself black, so each logo tile carries a light backing of its own:
  // `bg-brand-plate`, whose colour each theme supplies (white in `lofi`, a tint
  // of `black`'s blues in dark mode — see `style/tailwind.css`). `e2e-logos` is
  // seeded by `run-dioxus-gate.sh` and is deliberately *not* the artist the
  // links save/clear tests edit, since those wipe its rows mid-run.
  const spotifyLogo = 'img[src$="/music_services/Spotify.svg"]';
  await page.goto(`${base}/artists/e2e-logos`);
  await expect(page.locator(spotifyLogo)).toBeVisible();

  await page.emulateMedia({ colorScheme: "light" });
  const light = await measureLogoBacking(page, spotifyLogo);
  await page.emulateMedia({ colorScheme: "dark" });
  const dark = await measureLogoBacking(page, spotifyLogo);

  // Whatever the theme paints the page, the fixed ink must sit on light.
  expect(light.backingLuma, "the light theme needs a light backing").toBeGreaterThanOrEqual(180);
  expect(dark.backingLuma, "the dark theme needs a light backing").toBeGreaterThanOrEqual(180);
  // The colour-scheme emulation really did swap themes: `lofi` paints the page
  // near-white, `black` near-black. Without that, the check above would just be
  // measuring one theme twice.
  expect(light.pageLuma, "the light theme paints the page light").toBeGreaterThanOrEqual(200);
  expect(dark.pageLuma, "the dark theme paints the page dark").toBeLessThanOrEqual(60);
  // On a dark page the light has to come from the tile, not from the page.
  expect(dark.backingLuma - dark.pageLuma).toBeGreaterThanOrEqual(120);
  // The backing is themed rather than hardcoded white: `lofi` plates on white so
  // the tile disappears into that light page, while the dark theme plates with a
  // tint of its own blues — light enough to read, without glaring white tiles.
  expect(light.backingLuma, "the light theme plates on white").toBeGreaterThanOrEqual(250);
  expect(dark.backingLuma, "the dark theme plates with a tint, not white").toBeLessThan(250);
  const [tintRed, , tintBlue] = dark.backing;
  expect(tintBlue, "the dark theme's plate tints toward its own blues").toBeGreaterThan(tintRed);
});

test("nested admin tabs navigate and mark the active tab", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // `e2e-tabs` is seeded idempotently by end2end/run-dioxus-gate.sh; the
  // profile editor fetches the real row, so the header is the
  // artist name + "Profile" rather than the old placeholder.
  await page.goto(`${base}/admin/artist/e2e-tabs`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Profile");
  // The tabs are `role="tab"` links rendered by the AdminArtistRoot layout.
  await expect(page.getByRole("tab", { name: "Profile" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Releases" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/releases\/?$/);
  // The releases tab is a real list: the header names the artist.
  await expect(h1(page)).toHaveText("E2E Tabs Artist Releases");
  await expect(page.getByRole("tab", { name: "Releases" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Links" }).click();
  // (Links to routes with optional query segments render a trailing `?`.)
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/links\??$/);
  // The links tab's editor header names the artist.
  await expect(h1(page)).toHaveText("E2E Tabs Artist Links");
  await expect(page.getByRole("tab", { name: "Links" })).toHaveClass(/tab-active/);

  await page.getByRole("tab", { name: "Images" }).click();
  // The images tab carries the upload form's query codes too, so its link
  // renders the trailing `?` like the links tab above.
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/images\/?\??$/);
  // The images tab shows the header + the upload picker; the
  // javascript-off upload below and the client spec cover the upload itself.
  await expect(h1(page)).toHaveText("E2E Tabs Artist Images");
  await expect(page.getByRole("tab", { name: "Images" })).toHaveClass(/tab-active/);
  await expect(page.locator('input[name="file_to_upload"]')).toHaveCount(1);
});

test("the links editor gives its streaming logos a light panel", async ({ page }) => {
  // The card class is the saved/unsaved signal (accent versus secondary) and
  // `lofi` paints both dark, so the logo gets a light panel *inside* the card
  // rather than the card turning light. Every platform card renders whether or
  // not the artist has a saved row, so this needs no streaming rows seeded.
  requireAdmin();
  const spotifyLogo = 'img[src$="/music_services/Spotify.svg"]';
  await loginAs(page, "e2e-admin");
  await page.goto(`${base}/admin/artist/e2e-tabs/links`);
  await expect(page.locator(spotifyLogo)).toBeVisible();

  const backing = await measureLogoBacking(page, spotifyLogo);
  expect(backing.backingLuma, "the logo needs a light backing").toBeGreaterThanOrEqual(180);
  expect(backing.cardLuma, "the light theme paints the link card dark").not.toBeNull();
  expect(backing.cardLuma!).toBeLessThan(100);
});

test("an upload works without javascript, as a multipart form post", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // This server serves no wasm bundle, so clicking Upload is exactly
  // what a javascript-off browser does — a multipart POST to /forms/upload,
  // which stores the file and 303s back to the images tab with ?uploaded=1.
  await page.goto(`${base}/admin/artist/e2e-tabs/images`);
  await expect(h1(page)).toHaveText("E2E Tabs Artist Images");
  const stamp = Date.now();
  await page.setInputFiles('input[name="file_to_upload"]', {
    name: `e2e-nojs-${stamp}.png`,
    mimeType: "image/png",
    buffer: Buffer.from(PNG_1X1_BASE64, "base64"),
  });
  await page.getByRole("button", { name: "Upload" }).click();

  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/images\?uploaded=1$/);
  await expect(page.getByText("Image uploaded.")).toBeVisible();
  // The tab stays active despite the query codes, and the page now shows the
  // stored file (the handler attached it to the row before redirecting).
  await expect(page.getByRole("tab", { name: "Images" })).toHaveClass(/tab-active/);
  await expect(page.locator('img[src^="/uploads/artists/"]')).toHaveCount(1);
});

test("a track's audio uploads without javascript and gets a player", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // The gate seeds `e2e-audio-release`/`e2e-audio-track` under `e2e-tabs`, and
  // the track editor carries the audio uploader. With no wasm bundle on this
  // server, Upload is exactly what a javascript-off browser does: a multipart
  // POST to /forms/upload, which attaches the file and 303s back here.
  await page.goto(`${base}/admin/artist/e2e-tabs/release/e2e-audio-release/track/e2e-audio-track`);
  await expect(h1(page)).toHaveText("E2E Audio Track - E2E Audio Release - E2E Tabs Artist");
  // The audio slot's own accept list, which the config drives.
  await expect(page.locator('input[name="file_to_upload"]')).toHaveAttribute(
    "accept",
    ".mp3,.MP3,.wav,.WAV,.flac,.FLAC,.ogg,.OGG",
  );

  await page.setInputFiles('input[name="file_to_upload"]', {
    name: "e2e-audio.wav",
    mimeType: "audio/wav",
    buffer: emptyWav(),
  });
  await page.getByRole("button", { name: "Upload" }).click();

  await expect(page).toHaveURL(/\/track\/e2e-audio-track\?uploaded=1$/);
  // The slot names what it takes, unlike the image tabs.
  await expect(page.getByText("Audio uploaded.")).toBeVisible();
  // The handler attached the file before redirecting, so the page now serves a
  // player pointing at it.
  await expect(page.locator('audio[src^="/uploads/tracks/"]')).toHaveCount(1);
});

test("an upload with nothing chosen comes back with an error code", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // An untouched file input is not an upload: the browser sends an empty part
  // and the handler sends the user back with a code (`ui::upload`'s
  // upload_message turns it into the alert). Which code depends on what the
  // browser puts in the empty part, so the URL is the assertion here.
  await page.goto(`${base}/admin/artist/e2e-tabs/images`);
  await page.getByRole("button", { name: "Upload" }).click();
  await expect(page).toHaveURL(/\/admin\/artist\/e2e-tabs\/images\?bad_upload=/);
});

test("the admin dashboard renders cards and the side menu", async ({ page }) => {
  requireAdmin();
  await loginAs(page, "e2e-admin");

  // The navbar exposes the `admin` link only for `admin` sessions.
  await page.locator(".navbar summary").click();
  await page.getByRole("link", { name: "Admin" }).click();
  // The dashboard route carries optional query params, so its canonical
  // href (and thus the navbar link) ends in a bare `?`.
  await expect(page).toHaveURL(/\/admin\/?\??$/);
  await expect(page).toHaveTitle("Admin Dashboard");

  // Side menu (served with URLs built by the admin_menu service) and the
  // dashboard card row.
  await expect(page.locator("ul.menu").first()).toBeVisible();
  await expect(page.getByRole("link", { name: "Dashboard" })).toBeVisible();
  const label = await labelName(page);
  await expect(page.getByRole("heading", { level: 2, name: "Artists" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Pages" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Social Tips!" })).toBeVisible();
  // The housekeeping card sits at the bottom of every dashboard.
  await expect(page.getByRole("heading", { level: 2, name: "Housekeeping" })).toBeVisible();
  // The label card names the label (label_owner session).
  await expect(page.getByText(label).first()).toBeVisible();
  // With javascript off the preview is a plain post that 303s back to
  // `?ready=`; the gate seeds one page past the 90 day window (and earlier
  // runs may leave more), so the reload lists it server-side.
  await page.getByRole("button", { name: "Clean up" }).click();
  await expect(page).toHaveURL(/\/admin\?ready=\d+$/);
  await expect(page.getByText("ready to go for good")).toBeVisible();
  await expect(page.getByText("E2E Purge Page")).toBeVisible();
  // The listing names the upload files the purge orphans with them (the
  // gate seeds an aged release/track pair whose files sit under
  // UPLOAD_PATH; see run-dioxus-gate.sh), plus the sweep fixture no row
  // ever named.
  await expect(page.getByRole("heading", { level: 3, name: "Upload files" })).toBeVisible();
  await expect(page.getByText("releases/999999999-e2e-purge-release.jpg")).toBeVisible();
  await expect(page.getByText("tracks/999999999-e2e-purge-track.wav")).toBeVisible();
  await expect(page.getByText("artists/999999998-e2e-orphan.png")).toBeVisible();
  // The confirm step is offered with the count the preview found: files
  // are always present in the gate environment (the seed pair plus the
  // orphan fixture), so the label must count them alongside the items.
  await expect(page.getByRole("button", { name: /and \d+ files for good$/ })).toBeVisible();
});

test("the avatar dropdown is readable and above the page (light scheme)", async ({ page }) => {
  // Both bugs this pins are light-theme layout bugs in the navbar: the
  // dropdown inherits the navbar's white `text-primary-content` (which
  // rendered white on its white `bg-base-100` panel) and the dashboard's
  // cards are `position:relative`, so the dropdown needs its own z-index
  // or it paints *under* them. Playwright's default colour scheme is
  // light, which is where the contrast bug bites.
  requireAdmin();
  await loginAs(page, "e2e-admin");
  await page.goto(`${base}/admin`);
  await expect(page.getByRole("heading", { level: 2, name: "Housekeeping" })).toBeVisible();

  await page.locator(".navbar summary").click();
  const probe = await page.evaluate(() => {
    const ul = document.querySelector(".navbar details > ul");
    if (!ul) return null;
    const link = ul.querySelector("a");
    const style = getComputedStyle(link);
    const box = ul.getBoundingClientRect();
    const top = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
    return {
      linkColor: style.color,
      panelBackground: getComputedStyle(ul).backgroundColor,
      linkReachable: top !== null && ul.contains(top),
    };
  });
  expect(probe, "the avatar dropdown renders").not.toBeNull();
  expect(
    probe.linkColor,
    `dropdown text (${probe.linkColor}) must not be the panel colour (${probe.panelBackground})`,
  ).not.toBe(probe.panelBackground);
  expect(probe.linkReachable, "the dropdown must float above the cards").toBe(true);
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
  // handler carries no ?updated=1).
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

  // The release editor's Tracks sidebar links to the
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

  // The links editor renders one input per platform with the
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
