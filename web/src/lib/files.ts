// Opening and writing save files. Chromium browsers get the File System Access API, so a save
// is written back to the file it came from; elsewhere saving downloads a new file.

// The parts of the File System Access API used here (not in TypeScript's DOM types yet). They
// are optional because only Chromium browsers have them.
declare global {
	interface Window {
		showOpenFilePicker?: (options?: {
			types?: { description: string; accept: Record<string, string[]> }[];
			excludeAcceptAllOption?: boolean;
		}) => Promise<FileSystemFileHandle[]>;
	}
	interface FileSystemHandle {
		queryPermission?: (d: { mode: 'readwrite' }) => Promise<PermissionState>;
		requestPermission?: (d: { mode: 'readwrite' }) => Promise<PermissionState>;
	}
}

export interface OpenedFile {
	name: string;
	bytes: Uint8Array;
	/** Present when the file can be written back in place. */
	handle?: FileSystemFileHandle;
}

export const canWriteInPlace = (): boolean =>
	typeof window !== 'undefined' && typeof window.showOpenFilePicker === 'function';

/** Asks for a save with the browser's picker. `undefined` when the user cancels. */
export async function pickSave(): Promise<OpenedFile | undefined> {
	const picker = window.showOpenFilePicker;
	if (!picker) throw new Error('This browser has no file picker with write access.');
	let handles: FileSystemFileHandle[];
	try {
		handles = await picker({
			types: [
				{
					description: 'Pokémon save',
					accept: { 'application/octet-stream': ['.sav', '.dsv', '.bin'] }
				}
			]
		});
	} catch (e) {
		if (e instanceof DOMException && e.name === 'AbortError') return undefined;
		throw e;
	}
	const handle = handles[0];
	if (!handle) return undefined;
	const file = await handle.getFile();
	return { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()), handle };
}

const same = (a: Uint8Array, b: Uint8Array) =>
	a.length === b.length && a.every((v, i) => v === b[i]);

/**
 * Writes `bytes` over the file, then reads it back. Throws when permission is refused or the
 * read-back differs (the original is still in memory for a backup download).
 */
export async function writeInPlace(handle: FileSystemFileHandle, bytes: Uint8Array): Promise<void> {
	if (
		handle.queryPermission &&
		(await handle.queryPermission({ mode: 'readwrite' })) !== 'granted'
	) {
		if ((await handle.requestPermission?.({ mode: 'readwrite' })) !== 'granted')
			throw new Error('Permission to write the file was refused.');
	}
	const w = await handle.createWritable();
	await w.write(new Uint8Array(bytes));
	await w.close();
	const back = new Uint8Array(await (await handle.getFile()).arrayBuffer());
	if (!same(back, bytes))
		throw new Error('The file reads back differently. Download the original as a backup.');
}

/** Offers `bytes` as a download named `name`. */
export function download(name: string, bytes: Uint8Array): void {
	const url = URL.createObjectURL(
		new Blob([new Uint8Array(bytes)], { type: 'application/octet-stream' })
	);
	const a = document.createElement('a');
	a.href = url;
	a.download = name;
	document.body.append(a);
	a.click();
	a.remove();
	setTimeout(() => {
		URL.revokeObjectURL(url);
	}, 10_000);
}

/** `Platinum.sav` → `Platinum.original.sav`. */
export function backupName(name: string): string {
	const dot = name.lastIndexOf('.');
	return dot > 0 ? `${name.slice(0, dot)}.original${name.slice(dot)}` : `${name}.original`;
}
