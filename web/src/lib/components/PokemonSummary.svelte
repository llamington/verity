<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Sprite from '#lib/components/Sprite.svelte';
	import TypeBadge from '#lib/components/TypeBadge.svelte';
	import ItemIcon from '#lib/components/ItemIcon.svelte';
	import { typeSwatch } from '#lib/theme.js';
	import type { Summary, Named, Legality } from '#lib/verity.js';

	let {
		summary,
		legality,
		game
	}: {
		summary: Summary;
		legality?: Legality | undefined;
		/** The save's game id; with it, the summary shows the Pokémon's sprite. */
		game?: string | undefined;
	} = $props();

	// The save stores stats as HP, Atk, Def, Spe, SpA, SpD; show them in the games' order.
	const ORDER = [0, 1, 2, 4, 5, 3];
	const LABELS = ['HP', 'Atk', 'Def', 'SpA', 'SpD', 'Spe'];
	const MARKS = ['●', '▲', '■', '♥', '★', '♦'];
	const CONTEST = ['Cool', 'Beauty', 'Cute', 'Smart', 'Tough', 'Sheen'];

	const name = (n: Named) => n.name ?? `#${n.id}`;
	const mon = $derived(summary.mon);
	const species = $derived(mon.speciesName ?? `#${mon.species}`);
	const gender = $derived(['♂', '♀', ''][summary.gender] ?? '');
	const hex = (v: number, w: number) => v.toString(16).toUpperCase().padStart(w, '0');
	const tint = $derived(summary.types[0] ? typeSwatch(summary.types[0].id).bg : 'var(--muted)');
	const tint2 = $derived(summary.types[1] ? typeSwatch(summary.types[1].id).bg : tint);
	/** A stat bar's fill, 0 to 1. */
	const fill = (v: number, max: number) => Math.min(1, v / max);
	/** HP-bar colours: green, then yellow below half, red below a fifth. */
	const hpColour = (f: number) =>
		f >= 0.5 ? 'oklch(0.72 0.19 145)' : f >= 0.2 ? 'oklch(0.82 0.17 90)' : 'oklch(0.63 0.22 27)';
</script>

<div class="flex flex-col gap-4 text-sm" data-testid="summary">
	<div
		class="relative flex items-center gap-3 overflow-hidden rounded-2xl p-3"
		style:background-image="linear-gradient(135deg, color-mix(in oklch, {tint} 35%, transparent), color-mix(in
		oklch, {tint2} 20%, transparent))"
	>
		{#if game && !mon.egg}
			<div
				class="flex size-24 shrink-0 items-center justify-center rounded-full bg-card/70 shadow-inner ring-4 ring-white/50 dark:ring-white/10"
			>
				<Sprite {game} {mon} name={species} size={80} />
			</div>
		{/if}
		<div class="flex min-w-0 flex-col gap-1">
			<div class="flex flex-wrap items-baseline gap-2">
				<span class="font-pixel text-2xl font-bold">{mon.egg ? 'Egg' : mon.nickname}</span>
				{#if gender}<span
						class="text-lg font-bold {summary.gender ? 'text-female' : 'text-male'}"
						aria-label={summary.gender ? 'female' : 'male'}>{gender}</span
					>{/if}
				{#if mon.level != null && !mon.egg}
					<span class="font-bold text-muted-foreground">Lv. {mon.level}</span>
				{/if}
			</div>
			<div class="flex flex-wrap items-center gap-1">
				<span class="mr-1 text-muted-foreground"
					>{species}{mon.form ? ` (form ${mon.form})` : ''}</span
				>
				{#each summary.types as t (t.id)}<TypeBadge type={t} />{/each}
				{#if mon.shiny}<Badge class="bg-shiny text-black"
						><Sparkles aria-hidden="true" />Shiny</Badge
					>{/if}
				{#if !mon.checksumOk}<Badge variant="destructive">Bad checksum</Badge>{/if}
			</div>
		</div>
	</div>

	<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
		<dt class="text-muted-foreground">Nature</dt>
		<dd>{name(summary.nature)}</dd>
		<dt class="text-muted-foreground">Ability</dt>
		<dd>{name(summary.ability)}{summary.hiddenAbility ? ' (hidden)' : ''}</dd>
		<dt class="text-muted-foreground">Held item</dt>
		<dd class="flex items-center gap-1">
			{#if summary.heldItem.id}<ItemIcon name={summary.heldItem.name} size={20} />{name(
					summary.heldItem
				)}{:else}None{/if}
		</dd>
		<dt class="text-muted-foreground">Ball</dt>
		<dd class="flex items-center gap-1">
			<ItemIcon name={summary.ball.name} size={20} />{name(summary.ball)}
		</dd>
		<dt class="text-muted-foreground">Friendship</dt>
		<dd>{summary.friendship}</dd>
		<dt class="text-muted-foreground">PID</dt>
		<dd class="font-mono">{hex(summary.pid, 8)}</dd>
	</dl>

	<Separator />

	<div>
		<h3 class="mb-2 font-pixel text-base font-bold">Moves</h3>
		<ol class="grid gap-1.5" data-testid="moves">
			{#each summary.moves as m, i (i)}
				{@const check = legality?.moves[i]}
				<li
					class="flex flex-wrap items-center gap-2 rounded-lg border-l-4 bg-muted/60 px-2 py-1.5"
					style:border-left-color={m.moveType ? typeSwatch(m.moveType.id).bg : 'var(--border)'}
				>
					{#if m.id}
						{#if m.moveType}<TypeBadge type={m.moveType} />{/if}
						<span class="font-bold">{m.name ?? `#${m.id}`}</span>
						<span class="ml-auto text-xs font-bold text-muted-foreground">PP {m.pp}</span>
					{:else}
						<span class="text-muted-foreground">—</span>
					{/if}
					{#if check?.how}
						<span class={check.valid ? 'text-muted-foreground' : 'text-destructive'}
							>· {check.how}</span
						>
					{/if}
				</li>
			{/each}
		</ol>
	</div>

	<Separator />

	<div data-testid="stats">
		<div
			class="grid grid-cols-[2.5rem_2.5rem_1fr_2rem_2.5rem] items-center gap-x-2 gap-y-1 text-right tabular-nums"
		>
			<span></span>
			<span class="text-xs text-muted-foreground">{summary.stats ? 'Stat' : ''}</span>
			<span></span>
			<span class="text-xs text-muted-foreground">IV</span>
			<span class="text-xs text-muted-foreground">EV</span>
			{#each ORDER as i, n (i)}
				<span class="text-left text-xs font-bold text-muted-foreground">{LABELS[n]}</span>
				<span class="font-bold">{summary.stats?.[i] ?? ''}</span>
				{@const f = summary.stats
					? fill(summary.stats[i] ?? 0, 400)
					: fill(summary.ivs[i] ?? 0, 31)}
				<span
					class="h-2.5 overflow-hidden rounded-full border border-foreground/15 bg-muted"
					aria-hidden="true"
				>
					<span
						class="block h-full rounded-full"
						style:width="{f * 100}%"
						style:background-color={hpColour(f)}
					></span>
				</span>
				<span class={summary.ivs[i] === 31 ? 'font-bold text-legal' : ''}>{summary.ivs[i]}</span>
				<span class="text-muted-foreground">{summary.evs[i]}</span>
			{/each}
		</div>
	</div>

	<Separator />

	<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
		<dt class="text-muted-foreground">OT</dt>
		<dd>
			{summary.ot}
			<span class="text-muted-foreground">{summary.otFemale ? '♀' : '♂'}</span>
			· ID {String(summary.tid).padStart(5, '0')}
			<span class="text-muted-foreground">(SID {String(summary.sid).padStart(5, '0')})</span>
		</dd>
		<dt class="text-muted-foreground">Origin</dt>
		<dd>{summary.origin} · {summary.language}</dd>
		<dt class="text-muted-foreground">Met</dt>
		<dd>
			{name(summary.metLocation)}{summary.metLevel ? `, Lv. ${summary.metLevel}` : ''}, {summary.metDate}
		</dd>
		{#if summary.eggDate}
			<dt class="text-muted-foreground">Egg</dt>
			<dd>{name(summary.eggLocation)}, {summary.eggDate}</dd>
		{/if}
		{#if summary.fateful}
			<dt class="text-muted-foreground">Fateful</dt>
			<dd>Yes</dd>
		{/if}
		<dt class="text-muted-foreground">Markings</dt>
		<dd aria-label="Markings">
			{#each MARKS as m, i (i)}
				<span class={summary.markings[i] ? '' : 'opacity-25'}>{m}</span>
			{/each}
		</dd>
		<dt class="text-muted-foreground">Contest</dt>
		<dd class="tabular-nums">
			{CONTEST.map((c, i) => `${c} ${summary.contest[i]}`).join(' · ')}
		</dd>
		<dt class="text-muted-foreground">Ribbons</dt>
		<dd>{summary.ribbonCount}</dd>
	</dl>
</div>
