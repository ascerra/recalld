import { expect, type Page } from '@playwright/test';

/** Sign in with the launch token, open `path`, and wait for the shell. */
export async function open(page: Page, path = '/map') {
  const url = new URL(process.env.RECALLD_UI_URL!);
  await page.goto(url.toString());
  await page.goto(new URL(path, url.origin).toString());
  await expect(page.getByText('read-only · daemon connected')).toBeVisible();
}
