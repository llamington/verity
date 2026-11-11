// Loads the verity core (WASM) and the data blobs a save needs.
import init, { Session } from '#lib/wasm/verity_wasm.js';

let ready: Promise<unknown> | undefined;

async function fetchBlob(name: string): Promise<Uint8Array> {
	const res = await fetch(`/data/${name}.bin`);
	if (!res.ok) throw new Error(`data/${name}.bin: ${res.status}`);
	return new Uint8Array(await res.arrayBuffer());
}

/** Opens a save file and attaches the English strings and the game's data blob. */
export async function openSave(bytes: Uint8Array): Promise<Session> {
	ready ??= init();
	await ready;
	const session = new Session(bytes);
	const [strings, game] = await Promise.all([
		fetchBlob('strings_en'),
		fetchBlob(session.gameBlob())
	]);
	session.attach('strings', strings);
	session.attach('game', game);
	return session;
}

export type { Session };
export type { Mon, Overview, BoxView } from '#lib/wasm/verity_wasm.js';
