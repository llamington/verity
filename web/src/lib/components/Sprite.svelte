<script lang="ts">
	import CircleHelp from '@lucide/svelte/icons/circle-help';
	import { SvelteSet } from 'svelte/reactivity';
	import { spriteUrls, type SpriteMon } from '#lib/sprites.js';

	let {
		game,
		mon,
		name,
		size = 48
	}: {
		/** The save's game id (`pt`, `b2w2`, ...). */
		game: string;
		mon: SpriteMon;
		/** The species name, for the alt text and the fallback. */
		name: string;
		size?: number;
	} = $props();

	const urls = $derived(spriteUrls(game, mon));
	/** URLs that failed to load; the first other candidate shows, else the fallback. */
	const failed = new SvelteSet<string>();
	const src = $derived(urls.find((u) => !failed.has(u)));
</script>

{#if src}
	<img
		{src}
		alt={name}
		width={size}
		height={size}
		loading="lazy"
		decoding="async"
		class="shrink-0 [image-rendering:pixelated]"
		onerror={() => {
			failed.add(src);
		}}
		data-testid="sprite"
	/>
{:else}
	<span
		class="flex shrink-0 items-center justify-center text-muted-foreground"
		style:width="{size}px"
		style:height="{size}px"
		title={name}
		data-testid="sprite-fallback"
	>
		<CircleHelp aria-hidden="true" size={Math.round(size / 2)} />
		<span class="sr-only">{name}</span>
	</span>
{/if}
