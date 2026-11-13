import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';

// The Memory Link journey end to end: open a White 2 save → Memory Link → link a Black save →
// save → reopen and link again (nothing new). The rules are tested lower down
// (crates/verity/tests/link.rs, crates/verity-wasm/tests/editor.rs, the UnovaLink stories).
// Real saves are gitignored (corpus/); this skips without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

test('links a Black save into a White 2 save, saves and reopens', async ({ page }) => {
	test.skip(!existsSync(corpus + 'gen5_white2.sav'), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(corpus + 'gen4_platinum.sav');
	await expect(page.getByRole('link', { name: 'Bag' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Memory Link' })).toHaveCount(0);

	await page.getByTestId('file').setInputFiles(corpus + 'gen5_white2.sav');
	await page.getByRole('link', { name: 'Memory Link' }).click();
	await expect(page).toHaveURL(/\/app\/link$/);
	const screen = page.getByTestId('unova');
	await screen.getByLabel('Partner save').setInputFiles(corpus + 'gen5_black.sav');
	const plan = screen.getByTestId('unova-plan');
	await expect(plan).toContainText('Memory Link');
	await expect(plan).toContainText('Ready');
	await screen.getByRole('button', { name: 'Link' }).click();
	await expect(screen.getByTestId('unova-message')).toContainText('Linked with gen5_black.sav');
	await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');

	const [dl] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: 'Save (download)' }).click()
	]);
	await page.getByTestId('file').setInputFiles({
		name: 'linked.sav',
		mimeType: 'application/octet-stream',
		buffer: readFileSync(await dl.path())
	});
	await expect(page.getByText('Checksums OK')).toBeVisible();
	await page.getByRole('link', { name: 'Memory Link' }).click();
	await page
		.getByTestId('unova')
		.getByLabel('Partner save')
		.setInputFiles(corpus + 'gen5_black.sav');
	await expect(page.getByTestId('unova-plan')).toContainText('No new flashbacks.');
});
