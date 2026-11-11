// The open save and its file, shared by the /app layout and its routes.
import { checksOut, openSave, type Choices, type Overview, type Session } from '#lib/verity.js';
import { download, writeInPlace, type OpenedFile } from '#lib/files.js';
import { errorText } from '#lib/utils.js';

class Doc {
	session: Session | undefined = $state.raw();
	overview: Overview | undefined = $state.raw();
	fileName = $state('');
	/** Opening failed. */
	error = $state('');
	/** Where the save came from, for writing back in place (Chromium). */
	handle: FileSystemFileHandle | undefined = $state.raw();
	/** The file as opened, kept for a backup download. */
	original: Uint8Array | undefined = $state.raw();
	/** Changes made since the last save. */
	dirty = $state(false);
	/** Whether the checksums in the file as opened (or last saved) were valid. */
	fileChecksumsOk = $state(true);
	saveMessage = $state('');
	saveError = $state('');
	saving = $state(false);
	/** A file waiting for the user to confirm discarding unsaved changes. */
	pending: OpenedFile | undefined = $state.raw();
	/** Bumped after every change, so views re-read the save. */
	version = $state(0);
	/** The editor's pickers; loaded with the legality data. */
	choices: Choices | undefined = $state.raw();

	/** Opens `f`, first asking to discard unsaved changes. */
	open(f: OpenedFile) {
		if (this.dirty) this.pending = f;
		else void this.load(f);
	}

	async load(f: OpenedFile) {
		this.error = '';
		try {
			const next = await openSave(f.bytes.slice());
			this.session?.free();
			this.session = next;
			this.overview = next.overview();
			this.fileName = f.name;
			this.handle = f.handle;
			this.original = f.bytes;
			this.dirty = false;
			this.fileChecksumsOk = this.overview.checksumsOk;
			this.saveMessage = this.saveError = '';
			this.choices = undefined;
			this.version++;
		} catch (err) {
			this.error = `Couldn't open ${f.name}: ${errorText(err)}`;
		}
	}

	/** Call after anything wrote to the save. */
	changed() {
		if (!this.session) return;
		this.overview = this.session.overview();
		this.version++;
		this.dirty = true;
		this.saveMessage = '';
	}

	async save() {
		const session = this.session;
		if (!session) return;
		this.saving = true;
		this.saveError = this.saveMessage = '';
		try {
			const bytes = session.finalize();
			// Re-open what will be written: it must parse with valid checksums.
			if (!checksOut(bytes)) throw new Error('the finalized save fails its own checksums');
			if (this.handle) {
				await writeInPlace(this.handle, bytes);
				this.saveMessage = `Saved to ${this.fileName} and read back intact.`;
			} else {
				download(this.fileName, bytes);
				this.saveMessage = `Downloaded ${this.fileName}. Put it in place of your old save to use it.`;
			}
			session.markSaved();
			this.dirty = false;
			this.fileChecksumsOk = true;
			this.overview = session.overview();
		} catch (err) {
			this.saveError = errorText(err);
		} finally {
			this.saving = false;
		}
	}
}

/** The editor's one open save. */
export const doc = new Doc();
