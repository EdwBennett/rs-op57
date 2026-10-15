// @ts-check
const { test, expect } = require("@playwright/test");

// The score (site/content/piano/op57.mxl) is a single measure: three notes
// in the right hand's treble staff, three in the left hand's bass staff.
const NOTES = 6;
const STAVES = 2;

// Verovio is a ~7 MB download plus WASM start-up, so allow longer than
// Playwright's 5 s default for the score to appear.
const RENDER_TIMEOUT = 30_000;

/**
 * Loads the piano page and waits until Verovio has replaced the loading
 * message with an engraved score.
 * @param {import("@playwright/test").Page} page
 */
async function openScore(page) {
  const errors = [];
  page.on("pageerror", (err) => errors.push(err.message));
  page.on("console", (msg) => msg.type() === "error" && errors.push(msg.text()));

  await page.goto("/piano/");
  await expect(page.locator("#score-status")).toBeHidden({ timeout: RENDER_TIMEOUT });
  expect(errors).toEqual([]);
  return page.locator("#score > svg");
}

/**
 * Screenshots `locator` and counts its "ink" pixels: those whose luminance
 * differs clearly from the screenshot's background (its most common
 * colour). Done on real pixels, so it fails if the score is blank, hidden,
 * or drawn in a colour that blends into the page — black on a dark-mode
 * background, say — not just if the SVG elements are missing.
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} locator
 */
async function inkPixels(page, locator) {
  const png = (await locator.screenshot()).toString("base64");
  return page.evaluate(async (data) => {
    const blob = await (await fetch(`data:image/png;base64,${data}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = /** @type {OffscreenCanvasRenderingContext2D} */ (canvas.getContext("2d"));
    ctx.drawImage(bitmap, 0, 0);
    const px = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;

    const luma = (r, g, b) => (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
    const counts = new Map();
    for (let i = 0; i < px.length; i += 4) {
      const rgb = (px[i] << 16) | (px[i + 1] << 8) | px[i + 2];
      counts.set(rgb, (counts.get(rgb) ?? 0) + 1);
    }
    const bg = [...counts.entries()].sort((a, b) => b[1] - a[1])[0][0];
    const bgLuma = luma(bg >> 16, (bg >> 8) & 255, bg & 255);

    let ink = 0;
    for (let i = 0; i < px.length; i += 4) {
      if (Math.abs(luma(px[i], px[i + 1], px[i + 2]) - bgLuma) > 0.5) ink++;
    }
    return { ink, total: px.length / 4 };
  }, png);
}

/**
 * Counts the highlight-coloured (strongly red) pixels in a screenshot of
 * `locator` — #c4001d in light mode, #ff7b72 in dark (see piano.html).
 * @param {import("@playwright/test").Page} page
 * @param {import("@playwright/test").Locator} locator
 */
async function redPixels(page, locator) {
  const png = (await locator.screenshot()).toString("base64");
  return page.evaluate(async (data) => {
    const blob = await (await fetch(`data:image/png;base64,${data}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = /** @type {OffscreenCanvasRenderingContext2D} */ (canvas.getContext("2d"));
    ctx.drawImage(bitmap, 0, 0);
    const px = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
    let red = 0;
    for (let i = 0; i < px.length; i += 4) {
      if (px[i] - Math.max(px[i + 1], px[i + 2]) > 80) red++;
    }
    return red;
  }, png);
}

/**
 * Clicks the `index`-th note (0-based, in score order), at the centre of
 * its bounding box plus an optional offset — like a person would, with no
 * guarantee of landing on a painted pixel.
 * @param {import("@playwright/test").Page} page
 * @param {number} index
 * @param {{ dx?: number, dy?: number }} [offset]
 */
async function clickNote(page, index, { dx = 0, dy = 0 } = {}) {
  const box = await page.locator("#score g.note").nth(index).boundingBox();
  if (!box) throw new Error(`note ${index} has no bounding box`);
  await page.mouse.click(box.x + box.width / 2 + dx, box.y + box.height / 2 + dy);
}

// The third right-hand note. Its flat comes from the key signature; the
// exact wording is hello-core's (crates/hello-core/tests/describe_note.rs).
const A_FLAT_3 =
  "A♭3 — sixteenth note on the treble staff (staff 1), measure 1 (the opening upbeat), " +
  "2½ eighths into the measure. Key signature: 4 flats (A♭ major or F minor). " +
  "Time signature: 12/8. MIDI note 56, 207.7 Hz.";

for (const colorScheme of /** @type {const} */ (["light", "dark"])) {
  test.describe(`${colorScheme} mode`, () => {
    test.use({ colorScheme });

    test("Verovio engraves the score visibly", async ({ page }) => {
      const svg = await openScore(page);

      // Verovio's SVG for this score: one page, the expected staves and notes.
      await expect(svg).toBeVisible();
      await expect(page.locator("#page-info")).toHaveText("1 / 1");
      await expect(page.locator("#score g.staff")).toHaveCount(STAVES);
      await expect(page.locator("#score g.note")).toHaveCount(NOTES);

      // ...and it actually shows up as contrasting pixels on screen. Staff
      // lines, clefs and noteheads cover a few percent of the area.
      const { ink, total } = await inkPixels(page, svg);
      expect(ink / total).toBeGreaterThan(0.01);
    });

    test("clicking a note highlights and describes it", async ({ page }) => {
      const svg = await openScore(page);
      const notes = page.locator("#score g.note");
      const info = page.locator("#note-info");
      await expect(info).toHaveText("Click a note to describe it.");
      // Not 0: anti-aliased text can leave a few coloured fringe pixels.
      const redBefore = await redPixels(page, svg);
      const firstNoteRedBefore = await redPixels(page, notes.nth(0));

      await clickNote(page, 2);
      await expect(page.locator("#score g.note.selected")).toHaveCount(1);
      await expect(notes.nth(2)).toHaveClass(/\bselected\b/);
      // Same id as in hello-core's MEI fixture: xmlIdSeed keeps them stable.
      await expect(notes.nth(2)).toHaveAttribute("id", "xfggj11");
      await expect(info).toHaveText(A_FLAT_3);
      // The highlight really is drawn: only the selected note turns red.
      expect(await redPixels(page, svg)).toBeGreaterThan(redBefore + 20);
      expect(await redPixels(page, notes.nth(2))).toBeGreaterThan(20);
      expect(await redPixels(page, notes.nth(0))).toBe(firstNoteRedBefore);

      // Choosing another note moves the highlight and the description.
      await clickNote(page, 3);
      await expect(page.locator("#score g.note.selected")).toHaveCount(1);
      await expect(notes.nth(3)).toHaveClass(/\bselected\b/);
      await expect(info).toHaveText(/^D♭2 — quarter note on the bass staff \(staff 2\)/);
    });
  });
}

test("score fits a phone-width screen", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  const svg = await openScore(page);

  const box = await svg.boundingBox();
  expect(box).not.toBeNull();
  expect(box?.width).toBeGreaterThan(0);
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(375);

  // No sideways scrolling of the page or of the score itself.
  const overflow = await page.evaluate(() => {
    const score = /** @type {HTMLElement} */ (document.getElementById("score"));
    return {
      page: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      score: score.scrollWidth - score.clientWidth,
    };
  });
  expect(overflow).toEqual({ page: 0, score: 0 });

  await expect(page.locator("#score g.note")).toHaveCount(NOTES);
});

test("a tap near a note selects it on a phone-width screen", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  const svg = await openScore(page);
  const info = page.locator("#note-info");

  // Well off the note's own bounding box, but within the page's
  // CLICK_RADIUS: still selects it.
  const box = await page.locator("#score g.note").nth(2).boundingBox();
  if (!box) throw new Error("note has no bounding box");
  await page.mouse.click(box.x + box.width + 12, box.y + box.height / 2);
  await expect(info).toHaveText(A_FLAT_3);

  // Far from any note (the clefs at the left edge): nothing changes.
  const svgBox = await svg.boundingBox();
  if (!svgBox) throw new Error("score has no bounding box");
  await page.mouse.click(svgBox.x + 4, svgBox.y + svgBox.height - 4);
  await expect(page.locator("#score g.note.selected")).toHaveCount(1);
  await expect(info).toHaveText(A_FLAT_3);
});
