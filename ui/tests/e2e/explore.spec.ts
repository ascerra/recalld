import { expect, test } from '@playwright/test';
import { open } from '../support/ui';

test('lists memories, filters by phase and sorts via the URL', async ({ page }) => {
  await open(page, '/explore');
  await expect(page.getByText('135 memories')).toBeVisible();
  const rows = page.locator('tbody tr.row');
  await expect(rows.first()).toBeVisible();
  // Toggle Full and Summary off: only Ghosts remain, with "Summary dropped".
  const toolbar = page.getByRole('toolbar', { name: 'Filters' });
  await toolbar.getByRole('button', { name: 'Full', exact: true }).click();
  await toolbar.getByRole('button', { name: 'Summary', exact: true }).click();
  await expect(page).toHaveURL(/phase=3/);
  await expect(rows.first().getByRole('button')).toHaveText('Summary dropped');
  await page.goto(page.url().replace(/\?.*/, '?sort=summary'));
  await expect(page).toHaveURL(/sort=summary/);
  await expect(toolbar.getByText('Memory', { exact: false })).toBeVisible();
});

test('works from the keyboard alone', async ({ page }) => {
  await open(page, '/explore');
  const first = page.locator('tbody tr.row').first().getByRole('button');
  await first.focus();
  await page.keyboard.press('Enter');
  const panel = page.getByRole('complementary', { name: 'Selected memory' });
  await expect(panel).toBeVisible();
  const title = await panel.getByRole('heading').textContent();
  await page.keyboard.press('ArrowDown');
  await expect(panel.getByRole('heading')).not.toHaveText(title!);
  await expect(page.locator('tbody tr.row.selected')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(panel).toBeHidden();
});

test('the detail panel shows the decay curve and recent activity', async ({ page }) => {
  await open(page, '/explore?sort=summary');
  await page.getByRole('button', { name: 'Connection bonus keeps well-linked memories in Full' }).click();
  const panel = page.getByRole('complementary', { name: 'Selected memory' });
  await expect(panel.getByRole('img', { name: 'Forgetting curve for the selected memory' })).toBeVisible();
  await expect(panel.getByText(/^R \d\.\d\d · S /)).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Reinforce' })).toBeVisible();
  await panel.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Forget…' }).click();
  await expect(page.getByRole('alertdialog')).toContainText('Forget this memory?');
  await page.getByRole('button', { name: 'Keep it' }).click();
  await expect(page.getByRole('alertdialog')).toBeHidden();
});
