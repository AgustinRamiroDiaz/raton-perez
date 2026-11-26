import { test, expect } from "@playwright/test";
import { REAL_LLM, final, openPopup, pasted } from "./popup-fixture";

// Records the popup with a real LLM:
//   RATON_LLM_URL=https://raton-perez.dev.apps.particle41.ninja/v1 \
//   RATON_LLM_KEY=... bunx playwright test popup-demo
// The video lands in test-results/.

test.skip(!REAL_LLM, "set RATON_LLM_URL and RATON_LLM_KEY to record it");
// One network hiccup shouldn't lose the recording.
test.describe.configure({ retries: 1 });
test.use({
  viewport: { width: 420, height: 360 },
  // Playwright records at the viewport size, without device scaling.
  video: { mode: "on", size: { width: 420, height: 360 } },
});

test("demo", async ({ page }) => {
  test.setTimeout(120_000);
  const calls = await openPopup(page);
  const pause = () => page.waitForTimeout(2000);
  const run = async (name: string) => {
    const before = await final(page).inputValue();
    await page.getByRole("button", { name }).click();
    await expect(final(page)).not.toHaveValue(before, { timeout: 30_000 });
    await pause();
  };

  await pause();
  await run("Proofread");
  await run("Professional");
  await page
    .getByPlaceholder(/Describe a change/)
    .pressSequentially("Translate it to Argentine Spanish", { delay: 40 });
  const before = await final(page).inputValue();
  await page.getByRole("button", { name: "Apply" }).click();
  await expect(final(page)).not.toHaveValue(before, { timeout: 30_000 });
  await pause();

  await page.evaluate(() =>
    (window as unknown as { __emit: (e: string, p: string) => void }).__emit(
      "popup-text",
      "Notes from the standup. Maria finished the login page and starts on billing today. Diego is blocked on the staging database and needs access from DevOps. Sofia fixed the popup paste bug. The release is still planned for Friday.",
    ),
  );
  await pause();
  await run("Key Points");
  await run("Table");
  await page.getByRole("button", { name: "Insert" }).click();
  await expect.poll(() => pasted(calls).length).toBe(1);
});
