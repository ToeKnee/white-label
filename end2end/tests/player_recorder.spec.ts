import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * The desktop bar's page-side media recorder, pinned in a real browser.
 *
 * `play_recorder_js` (`src/ui/player.rs`) is how the native client knows how
 * much of a track a listener heard: the webview's `<audio>` cannot be read from
 * Rust, and the media events dioxus bridges carry no data at all. The bar hands
 * the script to dioxus's eval channel — which runs it as
 * `new AsyncFunction("dioxus", body)` — and files the play from what the script
 * sends back.
 *
 * Being a string inside the crate, it fails silently: a typo, or a brace that
 * lost its `{{` escape, leaves the window with no recorder and the counters
 * quiet. These tests run the crate's exact text and pin the two assumptions the
 * recorder rests on:
 *
 * - a **capture** listener on `document` reaches the bar's element, because
 *   media events do not bubble (without that, nothing is ever sent);
 * - the element's `src` attribute comes back verbatim, since that url is how
 *   the Rust side matches a reading to the play it belongs to.
 */

/** What the recorder sends Rust; the shape `PlayMediaRecord` reads. */
type MediaReading = { src: string; current: number; duration: number };

/** The page globals this spec uses, in the shape `page.evaluate` can see. */
type PageStore = {
  readings?: MediaReading[];
  /** What an install settled on, once it settles: the guard's `false`. */
  installAnswer?: unknown;
};

/**
 * The recorder body, lifted out of `src/ui/player.rs` and un-escaped the way
 * `format!` un-escapes it: the placeholder substituted, the doubled braces
 * collapsed. It throws if the shape it looks for has moved, so a rename in the
 * crate fails here loudly instead of passing vacuously.
 */
const recorderBody = (): { body: string; barId: string } => {
  const source = readFileSync(join(__dirname, "..", "..", "src", "ui", "player.rs"), "utf8");
  const id = /const BAR_AUDIO_ID: &str = "([^"]*)"/.exec(source)?.[1];
  const start = source.indexOf("fn play_recorder_js() -> String {");
  const raw = start < 0 ? undefined : /r#"([\s\S]*?)"#/.exec(source.slice(start))?.[1];
  if (!raw || !id) {
    throw new Error("play_recorder_js or BAR_AUDIO_ID is not where this spec expects — update the extraction");
  }
  const body = raw
    .replace(/\{BAR_AUDIO_ID:\?\}/g, JSON.stringify(id))
    .replace(/\{\{/g, "{")
    .replace(/\}\}/g, "}");
  return { body, barId: id };
};

/**
 * A tiny real WAV as a data url: PCM mono at 8 kHz, so a browser decodes it,
 * plays it and fires media events for it. The gate's own fixture audio is byte
 * stubs that never decode, so this spec has to bring something playable.
 */
const playableWav = (seconds: number, hertz: number): string => {
  const sampleRate = 8000;
  const sampleCount = Math.round(sampleRate * seconds);
  const dataSize = sampleCount * 2;
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + dataSize, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(sampleRate * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write("data", 36);
  header.writeUInt32LE(dataSize, 40);
  const samples = Buffer.alloc(dataSize);
  for (let index = 0; index < sampleCount; index += 1) {
    samples.writeInt16LE(Math.round(Math.sin((2 * Math.PI * hertz * index) / sampleRate) * 8000), index * 2);
  }
  return `data:audio/wav;base64,${Buffer.concat([header, samples]).toString("base64")}`;
};

/** Hands the page the bar's element and a second element that is not the bar. */
const barAndDecoy = (barId: string, barSrc: string, decoySrc: string) =>
  `<audio id="${barId}" src="${barSrc}" controls></audio>
   <audio id="not-the-bar" src="${decoySrc}" controls></audio>`;

/**
 * Runs the recorder in the page exactly as dioxus's query bridge does — inside
 * an async function with `dioxus` in scope — with a stub channel that collects
 * what the recorder sends. Both installs go through the same collector, so a
 * second install that really did register listeners would double the readings
 * and show up here. The install never settles (that is what keeps dioxus's
 * channel open), so only a guard's answer is recorded rather than awaited.
 */
const installRecorder = async (page: import("@playwright/test").Page, body: string) => {
  await page.evaluate((body) => {
    const store = window as unknown as PageStore;
    if (!store.readings) {
      store.readings = [];
    }
    const channel = {
      send: (reading: MediaReading) => store.readings?.push(reading),
      recv: () => new Promise<never>(() => {}),
    };
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    new AsyncFunction("dioxus", body)(channel).then((answer) => {
      store.installAnswer = answer;
    });
  }, body);
};

/** Everything the recorder has sent so far. */
const readingsOf = async (page: import("@playwright/test").Page): Promise<MediaReading[]> =>
  await page.evaluate(() => (window as unknown as PageStore).readings ?? []);

/**
 * Plays one element through to its end, muted so the browser allows it without
 * a gesture, and answers with where the element itself says it got to.
 */
const playToEnd = async (page: import("@playwright/test").Page, id: string) =>
  await page.evaluate(async (id) => {
    const media = document.getElementById(id) as HTMLAudioElement;
    media.muted = true;
    await new Promise<void>((resolve, reject) => {
      media.addEventListener("ended", () => resolve(), { once: true });
      media.addEventListener("error", () => reject(new Error(`${id} failed to load`)), { once: true });
      media.play().catch(() => reject(new Error(`${id} would not play`)));
    });
    return { current: media.currentTime, duration: media.duration };
  }, id);

test("the recorder follows the bar's element and names the file it heard", async ({ page }) => {
  const { body, barId } = recorderBody();
  const barSrc = playableWav(0.35, 440);
  const decoySrc = playableWav(0.5, 330);
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.setContent(barAndDecoy(barId, barSrc, decoySrc));
  await installRecorder(page, body);

  const played = await playToEnd(page, barId);
  expect(played.duration, "the WAV this spec builds should be a decodable file").toBeGreaterThan(0.3);
  expect(played.current, "and it should have reached its end").toBeGreaterThanOrEqual(played.duration - 0.02);

  const readings = await readingsOf(page);
  expect(readings.length, "a file played to its end produces several readings").toBeGreaterThanOrEqual(3);
  for (const reading of readings) {
    expect(reading.src, "every reading carries the element's own src, verbatim").toBe(barSrc);
    expect(reading.duration, "the length comes from the element").toBeCloseTo(played.duration, 2);
    expect(reading.current, `the position lies inside the file: ${JSON.stringify(reading)}`).toBeGreaterThanOrEqual(0);
    expect(reading.current, `the position lies inside the file: ${JSON.stringify(reading)}`).toBeLessThanOrEqual(
      reading.duration + 0.02,
    );
  }
  const last = readings[readings.length - 1];
  expect(
    last.current / last.duration,
    `the reading filed at the end of the file should reach the complete mark: ${JSON.stringify(last)}`,
  ).toBeGreaterThanOrEqual(0.9);

  // The bar's id is the recorder's filter: an element the bar does not load
  // must never be able to answer for a play.
  const heardBefore = readings.length;
  const decoy = await playToEnd(page, "not-the-bar");
  expect(decoy.current, "the decoy must really play, or the filter proves nothing").toBeGreaterThan(0.4);
  expect((await readingsOf(page)).length, "an element that is not the bar should add no readings").toBe(heardBefore);

  expect(pageErrors, `page errors:\n${pageErrors.join("\n")}`).toEqual([]);
});

test("a second install changes nothing the bar would file twice", async ({ page }) => {
  // The Rust side installs once, but the recorder's own guard is what stops a
  // re-mounted bar sending every event twice: two sets of listeners would look
  // like two plays of the same file. The comparison is one seek against the
  // next, so the file's first `durationchange` cannot skew it.
  const { body, barId } = recorderBody();
  await page.setContent(`<audio id="${barId}" src="${playableWav(0.35, 440)}" controls></audio>`);
  await installRecorder(page, body);

  // Plays, pauses, then seeks: one reportable move of the play head.
  const seek = async (to: number) =>
    await page.evaluate(
      async ({ id, to }) => {
        const media = document.getElementById(id) as HTMLAudioElement;
        media.muted = true;
        await media.play();
        media.pause();
        const seeked = new Promise<string>((resolve) =>
          media.addEventListener("seeked", () => resolve("seeked"), { once: true }),
        );
        media.currentTime = to;
        const settled = await Promise.race([
          seeked,
          new Promise<string>((resolve) => setTimeout(() => resolve("timed out"), 5000)),
        ]);
        return { settled, duration: media.duration };
      },
      { id: barId, to },
    );

  // The first seek warms the element up (its `durationchange` is part of the
  // first batch of readings, not part of a comparison), then two like-for-like
  // seeks are measured around the second install.
  const warmup = await seek(0.1);
  expect(warmup.duration, "the fixture should be decoded before it is seeked").toBeGreaterThan(0.3);
  const afterWarmup = (await readingsOf(page)).length;
  expect(afterWarmup, "a playing file should already be reported").toBeGreaterThan(0);

  const first = await seek(0.2);
  expect(first.settled, "the first seek should report itself").toBe("seeked");
  const firstSeek = (await readingsOf(page)).length - afterWarmup;
  expect(firstSeek, "one seek with one recorder installed is reported").toBeGreaterThan(0);

  installRecorder(page, body);
  await expect
    .poll(() => page.evaluate(() => (window as unknown as PageStore).installAnswer), { timeout: 5000 })
    .toBe(false);

  const second = await seek(0.3);
  expect(second.settled, "the second seek should report itself").toBe("seeked");
  const secondSeek = (await readingsOf(page)).length - afterWarmup - firstSeek;
  expect(secondSeek, "asking for the recorder twice should not double what one seek produces").toBe(firstSeek);
});
