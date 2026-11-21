import { expect, test } from '@playwright/test';
import { open } from '../support/ui';

type Hooks = Window & { __recalldSettled?: boolean; __recalldPositions?: () => { x: number; y: number }[] };

/** Median distance from each drawn memory to its nearest neighbor, in screen pixels. */
const medianGap = (page: import('@playwright/test').Page) =>
  page.evaluate(() => {
    const p = (window as Hooks).__recalldPositions!();
    const gaps = p.map((a, i) => Math.min(...p.filter((_, j) => j !== i).map((b) => Math.hypot(a.x - b.x, a.y - b.y))));
    gaps.sort((a, b) => a - b);
    return gaps[Math.floor(gaps.length / 2)];
  });

test('the spacing slider spreads memories further apart', async ({ page }) => {
  await open(page, '/map');
  await page.waitForFunction(() => (window as Hooks).__recalldSettled === true);
  const before = await medianGap(page);
  await page.evaluate(() => ((window as Hooks).__recalldSettled = false));
  const slider = page.getByRole('slider', { name: 'Spacing' });
  await expect(slider).toHaveValue('1');
  await slider.fill('2');
  await expect(page.getByText('2×')).toBeVisible();
  await page.waitForFunction(() => (window as Hooks).__recalldSettled === true, undefined, { timeout: 60_000 });
  expect(await medianGap(page)).toBeGreaterThan(before * 1.3);
  // Remembered across reloads.
  await page.reload();
  await expect(page.getByRole('slider', { name: 'Spacing' })).toHaveValue('2');
});
