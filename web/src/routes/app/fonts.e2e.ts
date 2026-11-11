import { expect, test, type Page } from '@playwright/test';
import { existsSync } from 'node:fs';

// Japanese and Korean names draw in Noto Sans JP/KR, which the browser fetches only for the
// characters on the page (unicode-range subsets). Real saves are gitignored; skips without them.
const corpus = new URL('../../../../corpus/real/', import.meta.url).pathname;
const english = corpus + 'pkhex-avalonia/gen4_platinum.sav';
const japanese = corpus + 'pkmds-blazor/Pokemon Heart Gold  (JP)old.sav';

function fontsFetched(page: Page): string[] {
	const fetched: string[] = [];
	page.on('request', (r) => {
		if (/noto-sans-(jp|kr)/.test(r.url())) fetched.push(r.url());
	});
	return fetched;
}

test('an English save fetches no CJK fonts beyond the gender signs', async ({ page }) => {
	test.skip(!existsSync(english), 'corpus not present');
	const fetched = fontsFetched(page);
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(english);
	await expect(page.getByTestId('party').getByTestId('mon').first()).toBeVisible();
	await page.waitForLoadState('networkidle');
	// Nunito has no ♂ or ♀ (Nidoran's names in box 1), so they come from one small Noto Sans JP
	// subset; nothing else does.
	expect(fetched.filter((u) => u.includes('noto-sans-kr'))).toEqual([]);
	expect(fetched.length).toBeLessThanOrEqual(1);
});

test('a Japanese save draws its names in Noto Sans JP', async ({ page }) => {
	test.skip(!existsSync(japanese), 'corpus not present');
	const fetched = fontsFetched(page);
	await page.goto('/app');
	await page.getByTestId('file').setInputFiles(japanese);
	const first = page.getByTestId('party').getByTestId('mon').first();
	await expect(first).toContainText(/[぀-ヿ一-鿿]/);
	await expect.poll(() => fetched.some((u) => u.includes('noto-sans-jp'))).toBe(true);
	expect(fetched.some((u) => u.includes('noto-sans-kr'))).toBe(false);
});
