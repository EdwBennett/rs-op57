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
