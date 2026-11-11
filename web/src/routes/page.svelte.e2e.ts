import { expect, test } from '@playwright/test';

test('the landing page introduces verity and opens the editor', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('verity');
	await expect(page.getByRole('heading', { name: 'What it does' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'PKHeX' })).toHaveAttribute(
		'href',
		'https://github.com/kwsch/PKHeX'
	);
	await page.getByRole('link', { name: 'Open a save' }).click();
	await expect(page).toHaveURL(/\/app$/);
});
