import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';

// The bag journey end to end: open → Bag → add, change, remove → save → reopen. The rules are
// tested lower down (crates/verity/tests/bag.rs, crates/verity-wasm/tests/editor.rs, the
// BagEditor stories). Real saves are gitignored (corpus/); this skips without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

for (const file of ['gen4_heartgold.sav', 'gen5_black.sav']) {
	test(`edits the bag of ${file}, saves and reopens`, async ({ page }) => {
		test.skip(!existsSync(corpus + file), 'corpus not present');
		await page.goto('/app');
		await page.getByTestId('file').setInputFiles(corpus + file);
		await page.getByRole('link', { name: 'Bag' }).click();
		await expect(page).toHaveURL(/\/app\/bag$/);

		const bag = page.getByTestId('bag');
		const items = bag.getByRole('list', { name: 'Items' });
		const before = await items.getByTestId('bag-item').count();
		const picker = bag.getByLabel('Add an item');
		const option = picker.locator('option').nth(1);
		const name = ((await option.textContent()) ?? '').trim();
		await picker.selectOption({ index: 1 });
		await bag.getByLabel('Count', { exact: true }).fill('12');
		await bag.getByRole('button', { name: 'Add' }).click();
		await expect(bag.getByTestId('bag-message')).toHaveText(`Added ${name}`);
		await expect(items.getByTestId('bag-item')).toHaveCount(before + 1);
		const count = items.getByLabel(`Count of ${name}`);
		await expect(count).toHaveValue('12');
		await count.fill('34');
		await count.blur();
		await expect(bag.getByTestId('bag-message')).toHaveText(`${name}: 34`);
		await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');

		// Remove the first item that was there before.
		await items
			.getByTestId('bag-item')
			.first()
			.getByRole('button', { name: /^Remove / })
			.click();
		await expect(items.getByTestId('bag-item')).toHaveCount(before);
		const kept = await items.getByTestId('bag-item').allTextContents();

		const [dl] = await Promise.all([
			page.waitForEvent('download'),
			page.getByRole('button', { name: 'Save (download)' }).click()
		]);
		await page.getByTestId('file').setInputFiles({
			name: 'bag.sav',
			mimeType: 'application/octet-stream',
			buffer: readFileSync(await dl.path())
		});
		await expect(page.getByText('Checksums OK')).toBeVisible();
		await page.getByRole('link', { name: 'Bag' }).click();
		const reopened = page.getByTestId('bag').getByRole('list', { name: 'Items' });
		await expect(reopened.getByLabel(`Count of ${name}`)).toHaveValue('34');
		await expect(reopened.getByTestId('bag-item')).toHaveText(kept);
	});
}
