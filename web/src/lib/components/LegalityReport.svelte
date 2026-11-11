<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import type { Legality } from '#lib/verity.js';

	let { legality, error = '' }: { legality: Legality | undefined; error?: string } = $props();

	const badMoves = $derived(legality?.moves.some((m) => !m.valid) ?? false);
</script>

<section class="flex flex-col gap-2" data-testid="legality" aria-live="polite">
	<div class="flex flex-wrap items-center gap-2">
		<h3 class="font-medium">Legality</h3>
		{#if error}
			<Badge variant="outline">Not checked</Badge>
		{:else if !legality}
			<Badge variant="outline">Checking…</Badge>
		{:else if legality.status === 'legal'}
			<Badge class="bg-emerald-600 text-white dark:bg-emerald-500">Legal</Badge>
		{:else if legality.status === 'illegal'}
			<Badge variant="destructive">Illegal</Badge>
		{:else}
			<Badge variant="outline">Not checked</Badge>
		{/if}
		{#if legality?.origin}
			<span class="text-sm text-muted-foreground">Origin: {legality.origin}</span>
		{/if}
	</div>
	{#if error}
		<p class="text-sm text-muted-foreground">{error}</p>
	{:else if legality?.reason}
		<p class="text-sm text-muted-foreground">Not checked: {legality.reason}.</p>
	{/if}
	{#if legality && (legality.checks.length || badMoves)}
		<ul class="flex flex-wrap gap-1" aria-label="Failed checks">
			{#each legality.checks as c (c)}
				<li><Badge variant="secondary">{c}</Badge></li>
			{/each}
			{#if badMoves}<li><Badge variant="secondary">Moves</Badge></li>{/if}
		</ul>
	{/if}
</section>
