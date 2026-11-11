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

/** Opens a save file and attaches what viewing and saving it need. */
export async function openSave(bytes: Uint8Array): Promise<Session> {
	ready ??= init();
	await ready;
	const session = new Session(bytes);
	// The game's data and move data: what viewing and saving need (see Editor::save_blobs).
	await Promise.all([load('strings', 'strings_en'), ...session.saveBlobs().map((b) => load(b))]);
	return session;
}

/** Fetches what legality checks on this save read (about 1-2 MB, cached after the first time). */
export async function loadLegalityData(session: Session): Promise<void> {
	await Promise.all(session.legalityBlobs().map((b) => load(b)));
}

/** Whether `bytes` open as a save with valid checksums (the read-back check before writing). */
export function checksOut(bytes: Uint8Array): boolean {
	try {
		const s = new Session(bytes.slice());
		const ok = s.overview().checksumsOk;
		s.free();
		return ok;
	} catch {
		return false;
	}
}

/** Fetches the Mystery Gift databases for this save's generation (about 0.5 MB for Gen 4). */
export async function loadGiftData(session: Session): Promise<void> {
	await Promise.all(session.giftBlobs().map((b) => load(b)));
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
	MoveCheck,
	Choices,
	EditRequest,
	Preview,
	IvFix,
	WildAdd,
	EggAdd,
	WildSlot,
	Gift
} from '#lib/wasm/verity_wasm.js';
