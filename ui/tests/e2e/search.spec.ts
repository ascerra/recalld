import { expect, test } from '@playwright/test';
import { open } from '../support/ui';

test('⌘K searches with recalld’s hybrid search and explains the top result', async ({ page }) => {
  await open(page, '/map');
  await page.keyboard.press('ControlOrMeta+k');
  const dialog = page.getByRole('dialog', { name: 'Search memories' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('Browsing won’t reinforce memories')).toBeVisible();
  await page.keyboard.type('connection bonus keeps well-linked memories');
  const best = dialog.getByRole('option').first();
  await expect(best).toContainText('Connection bonus keeps well-linked memories in Full');
  await expect(best).toContainText(/vector \d\.\d\d/);
  await expect(dialog.getByText(/\d+ results · recalld hybrid search/)).toBeVisible();
  // Opening a result keeps you where you are: on the map it is selected there.
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/map\?id=/);
  await expect(page.getByRole('complementary', { name: 'Selected memory' }).getByRole('heading')).toHaveText(
    'Connection bonus keeps well-linked memories in Full',
  );
});

test('a result opened from Explore stays in Explore', async ({ page }) => {
  await open(page, '/explore');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('connection bonus keeps well-linked memories');
  await expect(page.getByRole('option').first()).toContainText('Connection bonus keeps well-linked memories in Full');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/explore/);
  // Its row scrolls into view and opens in place (or, if its row isn't
  // loaded yet, its details show above the table).
  await expect(page.locator('tbody tr.row.selected button, .pinned h2').first()).toHaveText(
    'Connection bonus keeps well-linked memories in Full',
  );
  await expect(page.getByRole('region', { name: 'Memory details' })).toBeVisible();
});

test('/ opens the palette and Esc closes it', async ({ page }) => {
  await open(page, '/explore');
  await page.keyboard.press('/');
  await expect(page.getByRole('dialog', { name: 'Search memories' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Search memories' })).toBeHidden();
});

test('search starts from the page scope and filters on its own', async ({ page }) => {
  await open(page, '/explore');
  const filters = page.getByRole('toolbar', { name: 'Filters' });
  await page.getByRole('group', { name: 'Namespace scope' }).getByRole('button', { name: /topic-rust/ }).click();
  await page.keyboard.press('ControlOrMeta+k');
  const dialog = page.getByRole('dialog', { name: 'Search memories' });
  const chip = dialog.getByRole('button', { name: /topic-rust/ });
  await expect(chip).toBeVisible();
  // The menu opens above the palette and can be clicked.
  await chip.click();
  await page.getByRole('menuitemradio', { name: /All namespaces/ }).click();
  await expect(dialog.getByRole('button', { name: /All namespaces/ })).toBeVisible();
  // The page keeps its own scope, and reopening starts from it again.
  await page.getByRole('button', { name: 'Close search' }).click();
  await expect(dialog).toBeHidden();
  await expect(filters.getByRole('button', { name: /topic-rust/ })).toBeVisible();
  await page.keyboard.press('ControlOrMeta+k');
  await expect(dialog.getByRole('button', { name: /topic-rust/ })).toBeVisible();
});
