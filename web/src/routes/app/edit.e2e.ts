import { expect, test, type Page } from '@playwright/test';
import { existsSync } from 'node:fs';

// Mirrors the DS harness scripts ui-edit.json and ui-closest-ivs.json. Real saves are
// gitignored (corpus/); these tests skip without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

async function editFirst(page: Page, file: string) {
	test.skip(!existsSync(corpus + file), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(corpus + file);
	await page.getByTestId('party').getByTestId('mon').first().click();
	await expect(page.getByTestId('legality')).toContainText(/Legal|Illegal|Not checked/, {
		timeout: 15_000
	});
	await page.getByRole('button', { name: 'Edit', exact: true }).click();
	return page.getByTestId('editor');
}

test('legal mode refuses an unlearnable move, game-safe mode writes it', async ({ page }) => {
	const editor = await editFirst(page, 'gen4_platinum.sav');
	await editor.getByRole('tab', { name: 'Moves' }).click();
	await editor.getByLabel('Move 1').selectOption({ label: 'Spore' });
	await expect(editor.getByTestId('legality')).toContainText('Illegal');
	await expect(editor).toContainText("can't be learnt");
	await expect(editor.getByTestId('game-safe')).toContainText('OK');

	await editor.getByRole('button', { name: 'Apply' }).click();
	await expect(editor.getByTestId('edit-status')).toHaveText('Refused: illegal (legal mode)');

	await editor.getByRole('switch').click();
	await expect(editor).toContainText('Game-safe mode');
	await editor.getByRole('button', { name: 'Apply' }).click();
	await expect(page.getByTestId('editor').getByTestId('edit-status')).toHaveText('No changes yet.');

	await page.getByRole('button', { name: 'Summary' }).click();
	await expect(page.getByTestId('moves')).toContainText('Spore');
	await expect(page.getByTestId('legality')).toContainText('Illegal');
});

test('a character Gen 4 cannot store is refused before applying', async ({ page }) => {
	const editor = await editFirst(page, 'gen4_platinum.sav');
	await editor.getByLabel('Nickname').fill('Zap€');
	await expect(editor.getByTestId('game-safe')).toContainText("can't store");
	await expect(editor.getByRole('button', { name: 'Apply' })).toBeDisabled();
	await editor.getByLabel('Nickname').fill('Zappy');
	await expect(editor.getByTestId('game-safe')).toContainText('OK');
	await editor.getByRole('button', { name: 'Apply' }).click();
	await page.getByRole('button', { name: 'Summary' }).click();
	await expect(page.getByTestId('summary')).toContainText('Zappy');
	await expect(page.getByTestId('party').getByTestId('mon').first()).toContainText('Zappy');
});

test('closest legal IVs keep a wild catch legal', async ({ page }) => {
	const editor = await editFirst(page, 'gen4_platinum.sav');
	await editor.getByRole('tab', { name: 'Stats' }).click();
	for (const stat of ['HP', 'Atk', 'Def', 'SpA', 'SpD', 'Spe']) {
		await editor.getByLabel(`${stat} IV`).fill('31');
	}
	await expect(editor.getByTestId('legality')).toContainText('Illegal');
	await editor.getByRole('button', { name: 'Closest legal IVs' }).click();
	await expect(editor.getByTestId('iv-note')).toContainText(/legal spread/, { timeout: 30_000 });
	await expect(editor.getByTestId('legality')).toContainText('Legal');
	await editor.getByRole('button', { name: 'Apply' }).click();
	await expect(page.getByTestId('editor').getByTestId('edit-status')).toHaveText('No changes yet.');
});

test('edits a Gen 5 Pokémon', async ({ page }) => {
	const editor = await editFirst(page, 'gen5_white2.sav');
	await expect(editor.getByRole('button', { name: 'Closest legal IVs' })).toHaveCount(0);
	await editor.getByLabel('Friendship').fill('70');
	await expect(editor.getByTestId('legality')).toContainText('Legal');
	await editor.getByRole('button', { name: 'Apply' }).click();
	await page.getByRole('button', { name: 'Summary' }).click();
	await expect(page.getByTestId('summary')).toContainText(/Friendship\s*70/);
});
