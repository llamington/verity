<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import type { Gift } from '#lib/verity.js';

	let {
		gifts,
		deliver
	}: {
		/** The gifts offered to this save's game and language. */
		gifts: Gift[];
		/** Returns why it was refused, or nothing when delivered. */
		deliver: (gift: Gift, date: string) => string | undefined;
	} = $props();

	const today = () => {
		const d = new Date();
		const p = (n: number) => String(n).padStart(2, '0');
		return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
	};

	let query = $state('');
	let readyOnly = $state<boolean>(false);
	let date = $state(today());
	/** What happened to the last delivery, by gift key. */
	let results: Record<string, { ok: boolean; text: string }> = $state({});

	const key = (g: Gift) => `${g.source}:${g.record}`;
	const id = (g: Gift) => String(g.tid).padStart(5, '0');
	/** Releases of one event share a title; the OT and ID tell their buttons apart. */
	const label = (g: Gift) =>
		`Deliver ${g.title} (${g.gives}${g.ot ? `, OT ${g.ot}, ID ${id(g)}` : ''})`;
	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return gifts.filter(
			(g) =>
				// A card just delivered stays listed, so its result is seen.
				(!readyOnly || g.ready || key(g) in results) &&
				(!q || [g.title, g.gives, g.ot ?? '', id(g)].some((t) => t.toLowerCase().includes(q)))
		);
	});
	const readyCount = $derived(gifts.filter((g) => g.ready).length);

	function onDeliver(g: Gift) {
		const why = deliver(g, date);
		results[key(g)] = why ? { ok: false, text: why } : { ok: true, text: 'Delivered' };
	}
</script>

<div class="flex flex-col gap-3 text-sm" data-testid="gifts">
	<div class="flex flex-wrap items-end gap-3">
		<div class="flex min-w-48 flex-1 flex-col gap-1">
			<Label for="gift-search">Search</Label>
			<Input
				id="gift-search"
				type="search"
				placeholder="Event, Pokémon, OT or ID"
				bind:value={query}
				autocomplete="off"
			/>
		</div>
		<div class="flex flex-col gap-1">
			<Label for="gift-date">Received on</Label>
			<Input id="gift-date" type="date" min="2000-01-01" max="2099-12-31" bind:value={date} />
		</div>
		<label class="flex h-8 items-center gap-2">
			<input type="checkbox" class="accent-primary" bind:checked={readyOnly} /> Ready only
		</label>
	</div>
	<p class="text-muted-foreground" role="status">
		{gifts.length} gifts for this game and language, {readyCount} ready. Showing {shown.length}.
	</p>

	<ul class="flex flex-col divide-y rounded-lg border" aria-label="Gifts">
		{#each shown as g (key(g))}
			{@const result = results[key(g)]}
			<li class="flex flex-wrap items-center gap-x-4 gap-y-1 p-3" data-testid="gift">
				<div class="flex min-w-0 flex-1 flex-col gap-0.5">
					<span class="font-medium">{g.title}</span>
					<span class="text-muted-foreground">
						{g.gives}{g.gives !== g.kind ? ` · ${g.kind}` : ''}
						{#if g.ot}· OT {g.ot} · ID {id(g)}{/if}
						{#if g.region}· {g.region}{/if}
						{#if g.cardId}· Card {g.cardId}{/if}
					</span>
				</div>
				{#if result}
					<span class={result.ok ? 'text-emerald-700 dark:text-emerald-400' : 'text-destructive'}
						>{result.text}</span
					>
				{:else if !g.ready}
					<Badge variant="outline">{g.status}</Badge>
				{/if}
				<Button
					size="sm"
					variant="outline"
					disabled={!g.ready || result?.ok}
					onclick={() => {
						onDeliver(g);
					}}
					aria-label={label(g)}>Deliver</Button
				>
			</li>
		{:else}
			<li class="p-3 text-muted-foreground">No gifts match.</li>
		{/each}
	</ul>
	<p class="text-xs text-muted-foreground">
		A delivered card goes in the Wonder Card album and is marked as received, as the game does.
		Collect the gift from the deliveryman: in any Poké Mart in Gen 4, any Pokémon Center in Gen 5.
	</p>
</div>
