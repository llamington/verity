<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import Sprite from '#lib/components/Sprite.svelte';
	import type { Mon } from '#lib/verity.js';

	let {
		mon,
		game,
		onselect,
		onadd
	}: {
		mon: Mon | null | undefined;
		/** The save's game id; with it, the tile shows the Pokémon's sprite. */
		game?: string | undefined;
		onselect?: () => void;
		/** Makes an empty slot a button that adds a Pokémon. */
		onadd?: () => void;
	} = $props();

	const label = $derived(mon ? (mon.speciesName ?? `#${mon.species}`) : '');
	const nicknamed = $derived(!!mon && !mon.egg && mon.nicknamed);
	const tile = 'flex min-h-16 flex-col gap-1 rounded-md border bg-card p-2 text-card-foreground';
</script>

{#snippet body(mon: Mon)}
	<div class="flex items-center gap-2">
		{#if game}
			<Sprite {game} {mon} name={mon.egg ? 'Egg' : label} size={40} />
		{/if}
		<div class="flex min-w-0 flex-1 flex-col gap-0.5">
			<span class="truncate font-medium">{mon.egg ? 'Egg' : label}</span>
			{#if (mon.level != null && !mon.egg) || nicknamed}
				<span class="truncate text-xs text-muted-foreground">
					{#if mon.level != null && !mon.egg}Lv. {mon.level}{/if}
					{#if nicknamed}“{mon.nickname}”{/if}
				</span>
			{/if}
			<div class="flex flex-wrap gap-1">
				{#if mon.shiny}<Badge variant="secondary">Shiny</Badge>{/if}
				{#if !mon.checksumOk}<Badge variant="destructive">Bad checksum</Badge>{/if}
			</div>
		</div>
	</div>
{/snippet}

{#if mon && onselect}
	<button
		type="button"
		onclick={onselect}
		class="{tile} cursor-pointer text-left transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
		data-testid="mon"
	>
		{@render body(mon)}
	</button>
{:else if mon}
	<div class={tile} data-testid="mon">{@render body(mon)}</div>
{:else if onadd}
	<button
		type="button"
		onclick={onadd}
		class="flex min-h-16 cursor-pointer items-center justify-center rounded-md border border-dashed text-muted-foreground transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
		aria-label="Add a Pokémon here"
		data-testid="empty"><span aria-hidden="true">+</span></button
	>
{:else}
	<div class="min-h-16 rounded-md border border-dashed" aria-label="Empty slot"></div>
{/if}
