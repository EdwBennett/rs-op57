// @ts-check
const { test, expect } = require("@playwright/test");

test("homepage shows two consecutive primes", async ({ page }) => {
  await page.goto("/");

  // The number pair is random and only appears once the browser has
  // fetched and run the WASM module, which fills the two `prime`
  // placeholders written in site/content/_index.md (see
  // site/templates/components.html). So this exercises the whole
  // Zola-build-plus-WASM pipeline, not just that the page loads.
  // toHaveText retries until init() has finished.
  await expect(page.locator('[data-prime="1"]')).toHaveText(/^\d+$/);
  await expect(page.locator('[data-prime="2"]')).toHaveText(/^\d+$/);

  // The wording around the numbers comes from the Markdown, untouched.
  await expect(page.locator("main p").first()).toHaveText(
    /^Today's rs-op57 prime numbers are \d+ and \d+\.$/,
  );
});
