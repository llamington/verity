<script lang="ts">
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Plus from '@lucide/svelte/icons/plus';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import Sprite from '#lib/components/Sprite.svelte';
	import type { Mon } from '#lib/verity.js';

	let {
		mon,
		game,
		variant = 'party',
		onselect,
		onadd
	}: {
		mon: Mon | null | undefined;
		/** The save's game id; with it, the tile shows the Pokémon's sprite. */
		game?: string | undefined;
		/** `party`: a wide card. `box`: a PC box cell, sprite above the name. */
		variant?: 'party' | 'box';
		onselect?: () => void;
		/** Makes an empty slot a button that adds a Pokémon. */
		onadd?: () => void;
	} = $props();

	const label = $derived(mon ? (mon.speciesName ?? `#${mon.species}`) : '');
	const nicknamed = $derived(!!mon && !mon.egg && mon.nicknamed);
	const box = $derived(variant === 'box');
	const tile = $derived(
		box
			? 'relative flex min-h-24 flex-col items-center justify-center gap-0.5 rounded-xl border-2 border-white/70 bg-card/85 p-1.5 text-center text-card-foreground shadow-sm dark:border-white/10'
			: 'relative flex min-h-16 flex-col gap-1 rounded-xl border-2 border-t-4 border-t-primary/70 bg-card p-2 text-card-foreground shadow-sm'
	);
	const interactive =
		'cursor-pointer transition-[transform,box-shadow,border-color] hover:-translate-y-0.5 hover:border-primary/60 hover:shadow-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none active:translate-y-0';
	const empty = $derived(
		box
			? 'flex min-h-24 items-center justify-center rounded-xl border-2 border-dashed border-white/60 bg-white/25 dark:border-white/15 dark:bg-black/10'
			: 'flex min-h-16 items-center justify-center rounded-xl border-2 border-dashed'
	);
</script>

{#snippet marks(mon: Mon)}
	{#if mon.shiny}
		<span class="absolute top-1 right-1 text-shiny" title="Shiny">
			<Sparkles size={14} aria-hidden="true" /><span class="sr-only">Shiny</span>
		</span>
	{/if}
	{#if !mon.checksumOk}<Badge variant="destructive">Bad checksum</Badge>{/if}
{/snippet}

{#snippet body(mon: Mon)}
	{#if box}
		{#if game}
			<Sprite {game} {mon} name={mon.egg ? 'Egg' : label} size={48} />
		{/if}
		<span class="w-full truncate text-xs font-bold">{mon.egg ? 'Egg' : label}</span>
		{#if mon.level != null && !mon.egg}
			<span class="text-[0.7rem] leading-none font-bold text-muted-foreground">Lv. {mon.level}</span
			>
		{/if}
		{#if nicknamed}<span class="sr-only">“{mon.nickname}”</span>{/if}
		{@render marks(mon)}
	{:else}
		<div class="flex items-center gap-2">
			{#if game}
				<Sprite {game} {mon} name={mon.egg ? 'Egg' : label} size={48} />
			{/if}
			<div class="flex min-w-0 flex-1 flex-col gap-0.5">
				<span class="truncate font-bold">{mon.egg ? 'Egg' : label}</span>
				{#if nicknamed}<span class="truncate text-xs text-muted-foreground">“{mon.nickname}”</span
					>{/if}
				{#if mon.level != null && !mon.egg}<span
						class="self-start rounded-full bg-secondary px-1.5 text-xs font-bold text-secondary-foreground"
						>Lv. {mon.level}</span
					>{/if}
				{@render marks(mon)}
			</div>
		</div>
	{/if}
{/snippet}

{#if mon && onselect}
	<button
		type="button"
		onclick={onselect}
		class="{tile} {interactive} {box ? '' : 'text-left'}"
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
		class="{empty} group cursor-pointer text-muted-foreground transition-colors hover:border-primary hover:text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
		aria-label="Add a Pokémon here"
		data-testid="empty"
		><Plus
			size={18}
			aria-hidden="true"
			class="opacity-40 transition-opacity group-hover:opacity-100"
		/></button
	>
{:else}
	<div class={empty} aria-label="Empty slot"></div>
{/if}
