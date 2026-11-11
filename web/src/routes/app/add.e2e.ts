import { expect, test, type Page } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The add journey end to end: empty slot → wizard → Add → summary → save → reopen. The wizard's
// rules are tested lower down (crates/verity/tests/front_end.rs, crates/verity-wasm/tests/
// editor.rs, the AddPokemon stories). Real saves are gitignored; these skip without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

/** Opens `file` and the wizard on the first empty box slot. */
async function openWizard(page: Page, file: string) {
	test.skip(!existsSync(corpus + file), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(corpus + file);
	await expect(page.getByTestId('box-name')).toBeVisible();
	const empty = page.getByTestId('empty');
	for (let i = 0; i < 30 && (await empty.count()) === 0; i++) {
		await page.getByRole('button', { name: 'Next box' }).click();
	}
	const box = (await page.getByTestId('box-name').textContent()) ?? '';
	await empty.first().click();
	const wizard = page.getByTestId('add-pokemon');
	await expect(wizard).toBeVisible({ timeout: 15_000 });
	return { wizard, box };
}

/** Adds, then checks the drawer switched to the new Pokémon's summary with a legal verdict. */
async function addAndCheck(page: Page, species: string) {
	await page.getByTestId('add-pokemon').getByRole('button', { name: 'Add', exact: true }).click();
	await expect(page.getByTestId('summary')).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('summary')).toContainText(species);
	await expect(page.getByTestId('legality')).toContainText('Legal', { timeout: 15_000 });
	await expect(page.getByTestId('legality')).not.toContainText('Illegal');
	await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');
}

/** Saves by download and reopens the file: the checksums must hold and `box` must hold `species`. */
async function saveAndReopen(page: Page, box: string, species: string) {
	await page.keyboard.press('Escape');
	const [dl] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: 'Save (download)' }).click()
	]);
	const bytes = readFileSync(await dl.path());
	await page.getByTestId('file').setInputFiles({
		name: 'added.sav',
		mimeType: 'application/octet-stream',
		buffer: bytes
	});
	await expect(page.getByText('Checksums OK')).toBeVisible();
	const name = page.getByTestId('box-name');
	for (let i = 0; i < 30 && (await name.textContent()) !== box; i++) {
		await page.getByRole('button', { name: 'Next box' }).click();
	}
	await expect(name).toHaveText(box);
	await expect(page.getByTestId('mon').filter({ hasText: species })).not.toHaveCount(0);
}

test('adds a legal Platinum wild Pokémon and saves', async ({ page }) => {
	const { wizard, box } = await openWizard(page, 'gen4_platinum.sav');
	await wizard.getByLabel('Species').selectOption({ label: 'Bidoof' });
	await expect(wizard.getByLabel('Encounter').locator('option')).not.toHaveCount(1);
	await wizard.getByLabel('Encounter').selectOption({ index: 1 });
	await addAndCheck(page, 'Bidoof');
	await saveAndReopen(page, box, 'Bidoof');
});

test('adds to the end of a party with room', async ({ page }) => {
	const file = fileURLToPath(new URL('../../../../sd/saves/Pokemon Platinum.sav', import.meta.url));
	test.skip(!existsSync(file), 'test save not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(file);
	const party = page.getByTestId('party');
	await expect(party.getByTestId('mon').first()).toBeVisible();
	const before = await party.getByTestId('mon').count();
	expect(before).toBeLessThan(6);
	await party.getByTestId('empty').click();
	const wizard = page.getByTestId('add-pokemon');
	await expect(wizard).toBeVisible({ timeout: 15_000 });
	await expect(page.getByText(`Party slot ${before + 1}`)).toBeVisible();
	await wizard.getByLabel('Species').selectOption({ label: 'Starly' });
	await addAndCheck(page, 'Starly');
	await expect(page.getByText(`Party slot ${before + 1}`)).toBeVisible();
	await expect(party.getByTestId('mon')).toHaveCount(before + 1);
	await expect(party.getByTestId('mon').last()).toContainText('Starly');
});
