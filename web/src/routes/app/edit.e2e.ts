import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';

// The edit journey end to end: drawer → Edit → change → Apply → every view shows the result.
// The editor's rules are tested lower down (crates/verity-wasm/tests/editor.rs and the
// PokemonEditor stories). Real saves are gitignored (corpus/); this skips without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;

test('an applied edit shows in the summary, the tile and the save bar', async ({ page }) => {
	test.skip(!existsSync(corpus + 'gen4_platinum.sav'), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(corpus + 'gen4_platinum.sav');
	await page.getByTestId('party').getByTestId('mon').first().click();
	await expect(page.getByTestId('legality')).toContainText(/Legal|Illegal|Not checked/, {
		timeout: 15_000
	});
	await page.getByRole('button', { name: 'Edit', exact: true }).click();
	const editor = page.getByTestId('editor');
	await editor.getByLabel('Nickname').fill('Zappy');
	await expect(editor.getByTestId('game-safe')).toContainText('OK');
	await editor.getByRole('button', { name: 'Apply' }).click();
	await expect(page.getByTestId('editor').getByTestId('edit-status')).toHaveText('No changes yet.');

	await page.getByRole('button', { name: 'Summary' }).click();
	await expect(page.getByTestId('summary')).toContainText('Zappy');
	await expect(page.getByTestId('party').getByTestId('mon').first()).toContainText('Zappy');
	await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');
});
