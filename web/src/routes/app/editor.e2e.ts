import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';

// Real saves are gitignored (corpus/); these tests skip without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

for (const [file, game] of [
	['gen4_platinum.sav', 'Platinum'],
	['gen4_heartgold.sav', 'HeartGold / SoulSilver'],
	['gen5_black.sav', 'Black / White'],
	['gen5_white2.sav', 'Black 2 / White 2']
] as const) {
	test(`opens ${file}`, async ({ page }) => {
		test.skip(!existsSync(corpus + file), 'corpus not present');
		await page.goto('/app');
		await page.getByTestId('file').setInputFiles(corpus + file);
		await expect(page.getByText(`${game} · ${file}`)).toBeVisible();
		await expect(page.getByText(/Checksums (OK|bad)/)).toBeVisible();

		const party = page.getByTestId('party').getByTestId('mon');
		await expect(party.first()).toBeVisible();
		// Names and levels come from the fetched blobs, not placeholders.
		await expect(party.first()).not.toContainText('#');
		await expect(party.first()).toContainText(/Lv\. \d+|Egg/);

		const name = page.getByTestId('box-name');
		const first = await name.textContent();
		await page.getByRole('button', { name: 'Next box' }).click();
		await expect(name).not.toHaveText(first ?? '');
	});
}

for (const file of ['gen4_platinum.sav', 'gen5_white2.sav']) {
	test(`shows the summary and legality of a party Pokémon in ${file}`, async ({ page }) => {
		test.skip(!existsSync(corpus + file), 'corpus not present');
		await page.goto('/app');
		await page.getByTestId('file').setInputFiles(corpus + file);
		await page.getByTestId('party').getByTestId('mon').first().click();

		const summary = page.getByTestId('summary');
		await expect(summary).toBeVisible();
		await expect(summary).toContainText('Nature');
		await expect(summary.getByTestId('stats')).toContainText('IV');
		await expect(summary.getByTestId('moves').locator('li').first()).not.toContainText('#');

		// The legality blobs are fetched on first use; the verdict replaces "Checking…".
		const legality = page.getByTestId('legality');
		await expect(legality).toContainText(/Legal|Illegal|Not checked/, { timeout: 15_000 });
		await expect(legality).not.toContainText('Checking');
		await expect(legality).not.toContainText('data missing');

		await page.keyboard.press('Escape');
		await expect(summary).toBeHidden();
	});
}

test('rejects a file that is not a save', async ({ page }) => {
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles({
		name: 'nope.sav',
		mimeType: 'application/octet-stream',
		buffer: Buffer.alloc(1024)
	});
	await expect(page.getByRole('alert')).toContainText("Couldn't open nope.sav");
});
