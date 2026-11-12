<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { doc } from '#lib/editor.svelte.js';
	import { canWriteInPlace, pickSave } from '#lib/files.js';
	import { errorText } from '#lib/utils.js';

	interface Props {
		size?: 'sm' | 'lg';
		variant?: 'default' | 'secondary';
		class?: string;
		/** data-testid of the file input, which the e2e tests fill. */
		testid?: string;
		/** Runs once a save was picked (or picking failed), e.g. to go to the editor. */
		onopened?: () => void;
		children: Snippet;
	}

	let {
		size = 'sm',
		variant = 'default',
		class: className = '',
		testid,
		onopened,
		children
	}: Props = $props();

	let input: HTMLInputElement | undefined = $state();
	// Where the browser allows it, the picker keeps a handle so the save can be written in place.
	// Decided after mount: the landing page is prerendered, where there is no window.
	let inPlace = $state(false);
	onMount(() => {
		inPlace = canWriteInPlace();
	});

	async function onclick() {
		if (!inPlace) {
			input?.click();
			return;
		}
		try {
			const f = await pickSave();
			if (!f) return;
			doc.open(f);
		} catch (err) {
			doc.error = errorText(err);
		}
		onopened?.();
	}

	async function onFile(e: Event & { currentTarget: HTMLInputElement }) {
		const el = e.currentTarget;
		const file = el.files?.[0];
		el.value = '';
		if (!file) return;
		doc.open({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
		onopened?.();
	}
</script>

<Button {size} {variant} class={className} {onclick}>{@render children()}</Button>
<!-- The button opens this where there's no in-place picker; the e2e tests fill it directly (saves
     opened through it download rather than write back). -->
<input
	bind:this={input}
	type="file"
	accept=".sav,.dsv,.bin"
	class="sr-only"
	tabindex="-1"
	aria-hidden="true"
	onchange={onFile}
	data-testid={testid}
/>
