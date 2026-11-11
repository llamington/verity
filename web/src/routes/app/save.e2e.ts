import { expect, test, type Page } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';

// Saving, as the DS does it: finalize, whole-save check, write, read back. Mirrors ui-leave.json
// for the unsaved-changes warning. Real saves are gitignored; these tests skip without them.
const corpus = new URL('../../../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;
const PT = corpus + 'gen4_platinum.sav';
const W2 = corpus + 'gen5_white2.sav';
const REFUSED = new URL('../../../../sd/saves/Test-Save-White-2.sav', import.meta.url).pathname;

async function rename(page: Page, name: string) {
	await page.getByTestId('party').getByTestId('mon').first().click();
	await expect(page.getByTestId('legality')).toContainText(/Legal|Illegal|Not checked/, {
		timeout: 15_000
	});
	await page.getByRole('button', { name: 'Edit', exact: true }).click();
	const editor = page.getByTestId('editor');
	await editor.getByLabel('Nickname').fill(name);
	await editor.getByRole('button', { name: 'Apply' }).click();
	await expect(editor.getByTestId('edit-status')).toHaveText('No changes yet.');
	await page.keyboard.press('Escape');
}

test('downloads an edited save that opens with the edit and valid checksums', async ({ page }) => {
	test.skip(!existsSync(PT), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(PT);
	await expect(page.getByTestId('save-bar')).toContainText('No changes yet.');
	await rename(page, 'Saved');
	await expect(page.getByTestId('save-bar')).toContainText('Unsaved changes.');
	await expect(page.getByText('Checksums updated on save')).toBeVisible();

	const [dl] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: 'Save (download)' }).click()
	]);
	expect(dl.suggestedFilename()).toBe('gen4_platinum.sav');
	await expect(page.getByTestId('save-bar')).toContainText('Downloaded gen4_platinum.sav');
	const path = await dl.path();
	const bytes = readFileSync(path);
	expect(bytes.length).toBe(readFileSync(PT).length);

	await page.getByTestId('file').setInputFiles({
		name: 'saved.sav',
		mimeType: 'application/octet-stream',
		buffer: bytes
	});
	await expect(page.getByText('Checksums OK')).toBeVisible();
	await expect(page.getByTestId('party').getByTestId('mon').first()).toContainText('Saved');

	const [orig] = await Promise.all([
		page.waitForEvent('download'),
		(async () => {
			await rename(page, 'Again');
			await page.getByRole('button', { name: 'Download original' }).click();
		})()
	]);
	expect(orig.suggestedFilename()).toBe('saved.original.sav');
	expect(readFileSync(await orig.path()).equals(bytes)).toBe(true);
});

test('writes back in place through the File System Access API', async ({ page }) => {
	test.skip(!existsSync(W2), 'corpus not present');
	// An in-memory stand-in for the picker and the file handle.
	await page.addInitScript(
		(data: number[]) => {
			let stored = new Uint8Array(data);
			const handle = {
				kind: 'file',
				name: 'white2.sav',
				getFile: () => Promise.resolve(new File([stored], 'white2.sav')),
				queryPermission: () => Promise.resolve('granted'),
				createWritable: () => {
					let next = new Uint8Array();
					return Promise.resolve({
						write: (b: Uint8Array) => {
							next = new Uint8Array(b);
							return Promise.resolve();
						},
						close: () => {
							stored = next;
							return Promise.resolve();
						}
					});
				}
			};
			Object.assign(window, {
				showOpenFilePicker: () => Promise.resolve([handle]),
				__stored: () => stored
			});
		},
		Array.from(readFileSync(W2))
	);
	await page.goto('/app');
	await page.getByRole('button', { name: 'Open a save…' }).click();
	await expect(page.getByText('Black 2 / White 2 · white2.sav')).toBeVisible();
	await rename(page, 'InPlace');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByTestId('save-bar')).toContainText(
		'Saved to white2.sav and read back intact.'
	);
	const changed = await page.evaluate(
		(orig: number[]) =>
			(window as unknown as { __stored: () => Uint8Array })
				.__stored()
				.some((v, i) => v !== orig[i]),
		Array.from(readFileSync(W2))
	);
	expect(changed).toBe(true);
	await page.getByRole('button', { name: 'Open a save…' }).click();
	await expect(page.getByTestId('party').getByTestId('mon').first()).toContainText('InPlace');
	await expect(page.getByText('Checksums OK')).toBeVisible();
});

test('asks before discarding unsaved edits', async ({ page }) => {
	test.skip(!existsSync(PT) || !existsSync(W2), 'corpus not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(PT);
	await rename(page, 'Pending');
	await page.getByTestId('file').setInputFiles(W2);
	const dialog = page.getByRole('alertdialog');
	await expect(dialog).toContainText('Discard unsaved changes?');
	await dialog.getByRole('button', { name: 'Keep editing' }).click();
	await expect(page.getByTestId('party').getByTestId('mon').first()).toContainText('Pending');

	await page.getByTestId('file').setInputFiles(W2);
	await page.getByRole('alertdialog').getByRole('button', { name: 'Discard and open' }).click();
	await expect(page.getByText('Black 2 / White 2 · gen5_white2.sav')).toBeVisible();
});

test('refuses to save when the whole-save check fails', async ({ page }) => {
	test.skip(!existsSync(REFUSED), 'test save not present');
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(REFUSED);
	await rename(page, 'Nope');
	let downloaded = false;
	page.on('download', () => (downloaded = true));
	await page.getByRole('button', { name: 'Save (download)' }).click();
	await expect(page.getByTestId('save-bar')).toContainText('not saved: met location');
	expect(downloaded).toBe(false);
	await expect(page.getByTestId('save-bar')).toContainText(/box \d+, slot \d+/);
});
