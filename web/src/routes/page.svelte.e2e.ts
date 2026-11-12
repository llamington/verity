import { expect, test } from '@playwright/test';

test('the landing page introduces verity and opens a save in the editor', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('verity');
	await expect(page.getByRole('heading', { name: 'What it does' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'PKHeX' })).toHaveAttribute(
		'href',
		'https://github.com/kwsch/PKHeX'
	);
	await expect(page.getByRole('img', { name: /legality verdict/ })).toBeVisible();

	// The save is picked on the landing page and opens in the editor (this one isn't a save).
	await page.getByTestId('landing-file').setInputFiles({
		name: 'nope.sav',
		mimeType: 'application/octet-stream',
		buffer: Buffer.alloc(1024)
	});
	await expect(page).toHaveURL(/\/app$/);
	await expect(page.getByRole('alert')).toContainText("Couldn't open nope.sav");
});
