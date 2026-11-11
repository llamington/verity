// Copies the data blobs the web build fetches at runtime into static/data.
// The WASM module holds code only (PLAN P12); each game's blob is fetched on demand.
import { copyFileSync, mkdirSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const src = new URL('../../data/blobs/', import.meta.url).pathname;
const dst = new URL('../static/data/', import.meta.url).pathname;
// Game, strings and legality blobs (see verity::legality::verdict::blobs_for).
const wanted =
	/^(dp|pt|hgss|bw|b2w2|strings_[a-z]+|moves[45]|names[45]|enc_[a-z0-9]+|gifts[45]|g3_origin)\.bin$/;

mkdirSync(dst, { recursive: true });
let n = 0;
for (const f of readdirSync(src)) {
	if (wanted.test(f)) {
		copyFileSync(join(src, f), join(dst, f));
		n++;
	}
}
console.log(`copied ${n} data blobs to static/data`);
