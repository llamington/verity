// Copies the data blobs the web build fetches at runtime into static/data.
// The WASM module holds code only (PLAN P12); each game's blob is fetched on demand.
import { copyFileSync, mkdirSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const src = new URL('../../data/blobs/', import.meta.url).pathname;
const dst = new URL('../static/data/', import.meta.url).pathname;
const wanted = /^(dp|pt|hgss|bw|b2w2|strings_[a-z]+)\.bin$/;

mkdirSync(dst, { recursive: true });
let n = 0;
for (const f of readdirSync(src)) {
	if (wanted.test(f)) {
		copyFileSync(join(src, f), join(dst, f));
		n++;
	}
}
console.log(`copied ${n} data blobs to static/data`);
