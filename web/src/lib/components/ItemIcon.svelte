<script lang="ts">
	import { itemIconUrl } from '#lib/sprites.js';

	/** An item's icon; decorative, so a missing icon leaves a plain dot. */
	let { name, size = 24 }: { name: string | undefined; size?: number } = $props();

	const url = $derived(itemIconUrl(name));
	let failed = $state<string | undefined>();
</script>

{#if url && failed !== url}
	<img
		src={url}
		alt=""
		width={size}
		height={size}
		loading="lazy"
		decoding="async"
		class="shrink-0 [image-rendering:pixelated]"
		onerror={() => (failed = url)}
	/>
{:else}
	<span
		class="flex shrink-0 items-center justify-center"
		style:width="{size}px"
		style:height="{size}px"
		aria-hidden="true"><span class="size-2 rounded-full bg-muted-foreground/30"></span></span
	>
{/if}
