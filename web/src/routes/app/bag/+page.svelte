<script lang="ts">
	import BagEditor from '#lib/components/BagEditor.svelte';
	import { doc } from '#lib/editor.svelte.js';
	import type { Named, Pouch, Session } from '#lib/verity.js';

	// `doc.version` is read so the bag is re-read after every change.
	const pouches: Pouch[] = $derived(doc.session && doc.version >= 0 ? doc.session.bag() : []);

	/** Runs a bag change on the open save; refusals come back as text. */
	function change(edit: (s: Session) => string | undefined): string | undefined {
		if (!doc.session) return 'No save is open.';
		const why = edit(doc.session);
		if (!why) doc.changed();
		return why;
	}

	const addable = (pouch: number): Named[] => doc.session?.bagAddable(pouch) ?? [];
	const add = (pouch: number, item: number, count: number) =>
		change((s) => s.bagAdd(pouch, item, count));
	const setCount = (pouch: number, index: number, count: number) =>
		change((s) => s.bagSetCount(pouch, index, count));
	const remove = (pouch: number, index: number) => change((s) => s.bagRemove(pouch, index));
</script>

<svelte:head>
	<title>Bag · Verity editor</title>
</svelte:head>

<BagEditor {pouches} {addable} {add} {setCount} {remove} />
