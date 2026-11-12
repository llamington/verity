import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';

// After one visit the service worker has cached the site, so the editor works with no network.
const save = new URL('../../../../corpus/real/pkhex-avalonia/gen4_platinum.sav', import.meta.url)
	.pathname;

test('the editor works offline after one visit', async ({ page, context }) => {
	test.skip(!existsSync(save), 'corpus not present');
	await page.goto('/app');
	// The first install takes over this page once the site is cached.
	await page.waitForFunction(() => navigator.serviceWorker.controller !== null);

	await context.setOffline(true);
	await page.reload();
	await page.getByTestId('file').setInputFiles(save);
	const first = page.getByTestId('party').getByTestId('mon').first();
	await expect(first).toContainText(/Lv\. \d+/);
	// Legality needs the encounter and move blobs, also from the cache.
	await first.click();
	await expect(page.getByTestId('legality')).toContainText(/Legal|Illegal/, { timeout: 15_000 });
	await expect(page.getByTestId('legality')).not.toContainText("Couldn't load");
});

test('the site can be installed', async ({ page, request }) => {
	await page.goto('/');
	const href = await page.locator('link[rel="manifest"]').getAttribute('href');
	expect(href).toBeTruthy();
	const manifest: unknown = await (await request.get(href ?? '')).json();
	expect(manifest).toMatchObject({ name: 'Verity', start_url: '/app', display: 'standalone' });
	for (const icon of ['/icons/icon-192.png', '/icons/icon-512.png', '/icons/icon-maskable-512.png'])
		expect((await request.get(icon)).ok()).toBe(true);
});
