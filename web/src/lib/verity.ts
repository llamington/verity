// Loads the verity core (WASM) and the data blobs a save needs.
import init, { Session, attach, attached } from '#lib/wasm/verity_wasm.js';

let ready: Promise<unknown> | undefined;
const inflight = new Map<string, Promise<void>>();

/** Fetches `data/<file>.bin` once per page and attaches it as `name`. */
function load(name: string, file = name): Promise<void> {
	if (attached(name)) return Promise.resolve();
	let p = inflight.get(name);
	if (!p) {
		p = fetch(`/data/${file}.bin`)
			.then(async (res) => {
				if (!res.ok) throw new Error(`data/${file}.bin: ${res.status}`);
				attach(name, new Uint8Array(await res.arrayBuffer()));
			})
			.finally(() => inflight.delete(name));
		inflight.set(name, p);
	}
	return p;
}

/** Opens a save file and attaches the English strings and the game's data blob. */
export async function openSave(bytes: Uint8Array): Promise<Session> {
	ready ??= init();
	await ready;
	const session = new Session(bytes);
	await Promise.all([load('strings', 'strings_en'), load(session.gameBlob())]);
	return session;
}

/** Fetches what legality checks on this save read (about 1-2 MB, cached after the first time). */
export async function loadLegalityData(session: Session): Promise<void> {
	await Promise.all(session.legalityBlobs().map((b) => load(b)));
}

export type { Session };
export type {
	Mon,
	Overview,
	BoxView,
	Summary,
	Legality,
	SaveCheck,
	Named,
	MoveSlot,
	MoveCheck
} from '#lib/wasm/verity_wasm.js';
