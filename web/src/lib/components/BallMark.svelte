<script lang="ts">
	let {
		size = 24,
		outline = false,
		class: cls = ''
	}: {
		size?: number;
		/** Line art in the current colour, for watermarks. */
		outline?: boolean;
		class?: string;
	} = $props();
</script>

<!-- A ball mark for the logo; decorative. -->
{#if outline}
	<svg
		width={size}
		height={size}
		viewBox="0 0 32 32"
		aria-hidden="true"
		class="shrink-0 {cls}"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
	>
		<circle cx="16" cy="16" r="14" />
		<path d="M2 16h9.5M20.5 16H30" />
		<circle cx="16" cy="16" r="4.5" />
	</svg>
{:else}
	<svg
		width={size}
		height={size}
		viewBox="0 0 32 32"
		aria-hidden="true"
		class="shrink-0 {cls}"
		fill="none"
		stroke="var(--ball-band)"
		stroke-width="2.5"
	>
		<path d="M2.5 16a13.5 13.5 0 0 1 27 0Z" fill="var(--ball-top)" />
		<path d="M2.5 16a13.5 13.5 0 0 0 27 0Z" fill="white" />
		<circle cx="16" cy="16" r="4.5" fill="white" />
		<circle cx="16" cy="16" r="1.6" fill="var(--ball-band)" stroke="none" />
	</svg>
{/if}
