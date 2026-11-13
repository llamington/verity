// Captures the landing page screenshots into static/screenshots/. Needs the gitignored corpus
// saves and a preview server:
//   npm run build && npx vite preview --port 4173 &
//   npm run screenshots
//
// Sprites and item icons are blocked, so the images hold no Nintendo art (the app shows its
// placeholder instead): the repo ships none.
import { writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';

const BASE = 'http://localhost:4173';
const CORPUS = new URL('../../corpus/real/pkhex-avalonia/', import.meta.url).pathname;
const OUT = new URL('../static/screenshots/', import.meta.url).pathname;

/**
 * Waits for running animations (the sheet sliding in) to end or be cancelled.
 * @param {import('playwright').Page} page
 */
async function settled(page) {
	await page.evaluate(() => Promise.allSettled(document.getAnimations().map((a) => a.finished)));
}

/**
 * Screenshots the top of an element, at most `height` CSS pixels of it, with `pad` pixels of
 * the page around it.
 * @param {import('playwright').Page} page
 * @param {import('playwright').Locator} el
 * @param {string} name
 * @param {number} height
 * @param {number} [pad]
 */
async function shoot(page, el, name, height, pad = 0) {
	const box = await el.boundingBox();
	if (box === null) throw new Error(`${name}: element not visible`);
	const png = await page.screenshot({
		clip: {
			x: box.x - pad,
			y: box.y - pad,
			width: box.width + 2 * pad,
			height: Math.min(box.height, height) + 2 * pad
		}
	});
	await writeFile(`${OUT}${name}.webp`, await webp(page, png, 1));
	await writeFile(`${OUT}${name}-1x.webp`, await webp(page, png, 0.5));
	return png;
}

/**
 * Re-encodes a PNG as WebP at `scale` of its size, using the browser's encoder (the repo has
 * no image library).
 * @param {import('playwright').Page} page
 * @param {Buffer} png
 * @param {number} scale
 */
async function webp(page, png, scale) {
	const b64 = await page.evaluate(
		async ([src, s]) => {
			const img = new Image();
			img.src = src;
			await img.decode();
			const canvas = document.createElement('canvas');
			canvas.width = Math.round(img.naturalWidth * s);
			canvas.height = Math.round(img.naturalHeight * s);
			const ctx = canvas.getContext('2d');
			if (ctx === null) throw new Error('no 2d context');
			ctx.imageSmoothingQuality = 'high';
			ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
			return canvas.toDataURL('image/webp', 0.9).replace(/^data:[^,]*,/, '');
		},
		/** @type {[string, number]} */ ([`data:image/png;base64,${png.toString('base64')}`, scale])
	);
	return Buffer.from(b64, 'base64');
}

const browser = await chromium.launch();
// The service worker would serve cached sprites past the route block.
const context = await browser.newContext({
	viewport: { width: 1280, height: 1000 },
	deviceScaleFactor: 2,
	colorScheme: 'light',
	serviceWorkers: 'block'
});
const page = await context.newPage();
await page.route('**/gh/PokeAPI/sprites@*/**', (route) => route.abort());

await page.goto(`${BASE}/app`);
await page.getByTestId('file').setInputFiles(CORPUS + 'gen4_platinum.sav');
await page.getByTestId('party').getByTestId('mon').first().click();

const sheet = page.getByRole('dialog');
await page
	.getByTestId('legality')
	.getByText(/Legal|Illegal/)
	.first()
	.waitFor({ timeout: 15_000 });
await settled(page);
await shoot(page, sheet, 'summary', 700);

await sheet.getByRole('button', { name: 'Edit', exact: true }).click();
await settled(page);
await shoot(page, sheet, 'edit', 610);
await page.keyboard.press('Escape');

await page.getByRole('link', { name: 'Mystery Gifts' }).click();
const gifts = page.getByTestId('gifts');
await gifts.waitFor();
await settled(page);
// Link previews (og:image) want a PNG; the gift list is the widest shot.
await writeFile(`${OUT}social.png`, await shoot(page, gifts, 'gifts', 420, 16));

await browser.close();
