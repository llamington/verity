<script lang="ts">
	import { typeSwatch } from '#lib/theme.js';
	import type { Named } from '#lib/verity.js';

	let { type }: { type: Named } = $props();

	const swatch = $derived(typeSwatch(type.id));
</script>

<span
	class="inline-flex h-5 min-w-14 items-center justify-center rounded-full px-2 text-[0.7rem] font-bold tracking-wide uppercase shadow-[inset_0_-2px_0_rgb(0_0_0/0.15)]"
	style:background-color={swatch.bg}
	style:color={swatch.fg}>{type.name ?? `#${type.id}`}</span
>
