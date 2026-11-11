<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import type { Mon } from '#lib/verity.js';

	let { mon }: { mon: Mon | null | undefined } = $props();

	const label = $derived(mon ? (mon.speciesName ?? `#${mon.species}`) : '');
	const nicknamed = $derived(!!mon && !mon.egg && mon.nicknamed);
</script>

{#if mon}
	<div
		class="flex min-h-16 flex-col gap-1 rounded-md border bg-card p-2 text-card-foreground"
		data-testid="mon"
	>
		<div class="flex items-baseline justify-between gap-2">
			<span class="truncate font-medium">{mon.egg ? 'Egg' : label}</span>
			{#if mon.level != null && !mon.egg}
				<span class="shrink-0 text-xs text-muted-foreground">Lv. {mon.level}</span>
			{/if}
		</div>
		{#if nicknamed}
			<span class="truncate text-xs text-muted-foreground">“{mon.nickname}”</span>
		{/if}
		<div class="flex flex-wrap gap-1">
			{#if mon.shiny}<Badge variant="secondary">Shiny</Badge>{/if}
			{#if !mon.checksumOk}<Badge variant="destructive">Bad checksum</Badge>{/if}
		</div>
	</div>
{:else}
	<div class="min-h-16 rounded-md border border-dashed" aria-label="Empty slot"></div>
{/if}
