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
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/explore\?id=/);
  await expect(page.getByRole('complementary', { name: 'Selected memory' }).getByRole('heading')).toHaveText(
    'Connection bonus keeps well-linked memories in Full',
  );
});

test('/ opens the palette and Esc closes it', async ({ page }) => {
  await open(page, '/explore');
  await page.keyboard.press('/');
  await expect(page.getByRole('dialog', { name: 'Search memories' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Search memories' })).toBeHidden();
});
