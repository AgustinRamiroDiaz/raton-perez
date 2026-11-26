import { test, expect } from "@playwright/test";
import {
  FAILING_TEXT,
  REAL_LLM,
  TRANSCRIPT,
  final,
  openPopup,
  pasted,
} from "./popup-fixture";

test.describe("Writing-tools popup", () => {
  test.skip(REAL_LLM, "runs with canned responses only");

  test("lists the writing-tools actions without the dictation prompt", async ({
    page,
  }) => {
    await openPopup(page);
    for (const name of ["Proofread", "Professional", "Key Points", "Table"])
      await expect(page.getByRole("button", { name })).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Improve Transcriptions" }),
    ).toHaveCount(0);
  });

  test("each action runs on the original text", async ({ page }) => {
    await openPopup(page);
    await page.getByRole("button", { name: "Proofread" }).click();
    await expect(final(page)).toHaveValue(`[Proofread] ${TRANSCRIPT}`);
    await page.getByRole("button", { name: "Table" }).click();
    await expect(final(page)).toHaveValue(`[Table] ${TRANSCRIPT}`);
    await page.getByRole("button", { name: "Original" }).click();
    await expect(final(page)).toHaveValue(TRANSCRIPT);
  });

  test("applies a custom instruction", async ({ page }) => {
    const calls = await openPopup(page);
    await page.getByPlaceholder(/Describe a change/).fill("make it rhyme");
    await page.getByRole("button", { name: "Apply" }).click();
    await expect(final(page)).toHaveValue(`[make it rhyme] ${TRANSCRIPT}`);
    const run = calls.find((c) => c.cmd === "popup_run_prompt");
    expect(run?.args).toMatchObject({
      promptId: null,
      custom: "make it rhyme",
    });
  });

  test("keyboard: Ctrl+N runs the Nth action, Ctrl+Enter inserts", async ({
    page,
  }) => {
    const calls = await openPopup(page);
    await page.keyboard.press("Control+2");
    await expect(final(page)).toHaveValue(`[Professional] ${TRANSCRIPT}`);
    await page.keyboard.press("Control+Enter");
    await expect
      .poll(() => pasted(calls))
      .toEqual([`[Professional] ${TRANSCRIPT}`]);
  });

  test("Insert pastes the edited Final text", async ({ page }) => {
    const calls = await openPopup(page);
    await final(page).fill("edited by hand");
    await page.getByRole("button", { name: "Insert" }).click();
    await expect.poll(() => pasted(calls)).toEqual(["edited by hand"]);
  });

  test("a new transcript replaces the old one", async ({ page }) => {
    await openPopup(page);
    await page.getByRole("button", { name: "Proofread" }).click();
    await expect(final(page)).toHaveValue(`[Proofread] ${TRANSCRIPT}`);
    await page.evaluate(() =>
      (window as unknown as { __emit: (e: string, p: string) => void }).__emit(
        "popup-text",
        "a second dictation",
      ),
    );
    await expect(final(page)).toHaveValue("a second dictation");
  });

  test("shows the backend error when an action fails", async ({ page }) => {
    await openPopup(page, FAILING_TEXT);
    await page.getByRole("button", { name: "Proofread" }).click();
    await expect(page.getByText("The AI request failed.")).toBeVisible();
    await expect(final(page)).toHaveValue(FAILING_TEXT);
  });
});
