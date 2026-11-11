import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';

// The Mystery Gift journey end to end: open → Mystery Gifts → deliver → save → reopen. The rules
// are tested lower down (crates/verity/tests/front_end.rs, crates/verity-wasm/tests/editor.rs,
// the GiftList stories). Real saves are gitignored (corpus/); this skips without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

for (const file of ['gen4_platinum.sav', 'gen5_white2.sav']) {
	test(`delivers a gift to ${file}, saves and reopens`, async ({ page }) => {
		test.skip(!existsSync(corpus + file), 'corpus not present');
		await page.goto('/app');
		await page.getByTestId('file').setInputFiles(corpus + file);
		await page.getByRole('link', { name: 'Mystery Gifts' }).click();
		await expect(page).toHaveURL(/\/app\/gifts$/);
		await expect(page.getByRole('link', { name: 'Mystery Gifts' })).toHaveAttribute(
			'aria-current',
			'page'
		);

		const list = page.getByTestId('gifts');
		await expect(list).toBeVisible({ timeout: 15_000 });
		await list.getByLabel('Ready only').check();
		const first = list.getByTestId('gift').first();
		const title = (await first.locator('span.font-medium').textContent()) ?? '';
		await first.getByRole('button').click();
		await expect(first).toContainText('Delivered');
		await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');

		// The save stays open across sections.
		await page.getByRole('link', { name: 'Boxes' }).click();
		await expect(page.getByTestId('party').getByTestId('mon').first()).toBeVisible();

		const [dl] = await Promise.all([
			page.waitForEvent('download'),
			page.getByRole('button', { name: 'Save (download)' }).click()
		]);
		await page.getByTestId('file').setInputFiles({
			name: 'gifted.sav',
			mimeType: 'application/octet-stream',
			buffer: readFileSync(await dl.path())
		});
		await expect(page.getByText('Checksums OK')).toBeVisible();
		await page.getByRole('link', { name: 'Mystery Gifts' }).click();
		const reopened = page.getByTestId('gifts');
		await expect(reopened).toBeVisible({ timeout: 15_000 });
		await reopened.getByLabel('Search').fill(title);
		await expect(reopened.getByTestId('gift').first()).not.toContainText('Ready');
		await expect(reopened.getByTestId('gift').first().getByRole('button')).toBeDisabled();
	});
}
