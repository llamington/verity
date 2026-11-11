// Builds the WASM core into a scratch folder, then copies only changed files into src/lib/wasm.
// wasm-pack empties its output folder first, which breaks a running `vite dev` (every page 500s
// until a restart) whenever tests or a build run alongside it.
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const web = new URL('..', import.meta.url).pathname;
const tmp = join(web, 'node_modules/.cache/verity-wasm');
const dst = join(web, 'src/lib/wasm');

execFileSync(
	'wasm-pack',
	[
		'build',
		'../crates/verity-wasm',
		'--target',
		'web',
		// CI only needs the bindings and their types; a dev build skips LTO and wasm-opt.
		process.env.VERITY_WASM_DEV ? '--dev' : '--release',
		'--out-dir',
		tmp,
		'--no-pack'
	],
	{ cwd: web, stdio: 'inherit' }
);
mkdirSync(dst, { recursive: true });
let n = 0;
for (const f of readdirSync(tmp)) {
	const from = join(tmp, f);
	const to = join(dst, f);
	if (existsSync(to) && readFileSync(from).equals(readFileSync(to))) continue;
	copyFileSync(from, to);
	n++;
}
console.log(`wasm: ${n} file(s) updated in src/lib/wasm`);
