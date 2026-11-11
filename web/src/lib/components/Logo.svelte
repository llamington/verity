<script lang="ts">
	import mark from '#lib/assets/favicon.svg';

	/** verity's mark (assets/logo/mark.py). Use multiples of 16 for `size` so its pixels stay even; `tile` sets it on a cream square for red backgrounds. */
	let {
		size = 24,
		tile = false,
		class: cls = ''
	}: { size?: number; tile?: boolean; class?: string } = $props();
</script>

{#if tile}
	<span
		class="inline-flex shrink-0 items-center justify-center rounded-[22%] bg-[#F8F4EA] shadow-[inset_0_-2px_0_rgb(0_0_0/0.15)] {cls}"
		style:width="{Math.round(size * 1.25)}px"
		style:height="{Math.round(size * 1.25)}px"
	>
		<img src={mark} alt="" width={size} height={size} />
	</span>
{:else}
	<img src={mark} alt="" width={size} height={size} class="shrink-0 {cls}" />
{/if}
