<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import Sprite from '#lib/components/Sprite.svelte';
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
</script>

<div class="flex flex-col gap-4 text-sm" data-testid="summary">
	<div class="flex items-center gap-3">
		{#if game && !mon.egg}
			<Sprite {game} {mon} name={species} size={80} />
		{/if}
		<div class="flex min-w-0 flex-col gap-1">
			<div class="flex flex-wrap items-baseline gap-2">
				<span class="text-lg font-semibold">{mon.egg ? 'Egg' : mon.nickname}</span>
				{#if gender}<span aria-label={summary.gender ? 'female' : 'male'}>{gender}</span>{/if}
				{#if mon.level != null && !mon.egg}
					<span class="text-muted-foreground">Lv. {mon.level}</span>
				{/if}
			</div>
			<div class="flex flex-wrap items-center gap-1">
				<span class="text-muted-foreground">{species}{mon.form ? ` (form ${mon.form})` : ''}</span>
				{#each summary.types as t (t.id)}<Badge variant="outline">{name(t)}</Badge>{/each}
				{#if mon.shiny}<Badge variant="secondary">Shiny</Badge>{/if}
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
		<dd>{summary.heldItem.id ? name(summary.heldItem) : 'None'}</dd>
		<dt class="text-muted-foreground">Ball</dt>
		<dd>{name(summary.ball)}</dd>
		<dt class="text-muted-foreground">Friendship</dt>
		<dd>{summary.friendship}</dd>
		<dt class="text-muted-foreground">PID</dt>
		<dd class="font-mono">{hex(summary.pid, 8)}</dd>
	</dl>

	<Separator />

	<div>
		<h3 class="mb-1 font-medium">Moves</h3>
		<ol class="flex flex-col gap-1" data-testid="moves">
			{#each summary.moves as m, i (i)}
				{@const check = legality?.moves[i]}
				<li class="flex flex-wrap items-center gap-2">
					{#if m.id}
						<span class="font-medium">{m.name ?? `#${m.id}`}</span>
						{#if m.moveType}<Badge variant="outline">{name(m.moveType)}</Badge>{/if}
						<span class="text-muted-foreground">PP {m.pp}</span>
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

	<table class="w-full text-right tabular-nums" data-testid="stats">
		<thead class="text-muted-foreground">
			<tr>
				<th class="text-left font-normal"></th>
				{#each LABELS as l (l)}<th class="font-normal">{l}</th>{/each}
			</tr>
		</thead>
		<tbody>
			{#if summary.stats}
				<tr>
					<th class="text-left font-normal text-muted-foreground">Stat</th>
					{#each ORDER as i (i)}<td>{summary.stats[i]}</td>{/each}
				</tr>
			{/if}
			<tr>
				<th class="text-left font-normal text-muted-foreground">IV</th>
				{#each ORDER as i (i)}<td>{summary.ivs[i]}</td>{/each}
			</tr>
			<tr>
				<th class="text-left font-normal text-muted-foreground">EV</th>
				{#each ORDER as i (i)}<td>{summary.evs[i]}</td>{/each}
			</tr>
		</tbody>
	</table>

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
