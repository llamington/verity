<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/native-select/index.js';
	import { errorText } from '#lib/utils.js';
	import type { Choices, EggAdd, Named, WildAdd, WildSlot } from '#lib/verity.js';

	let {
		versions,
		choices,
		generation,
		wildSpecies,
		wildEncounters,
		eggSpecies,
		addWild,
		addEgg
	}: {
		/** The save's own game first. */
		versions: Named[];
		choices: Choices;
		generation: number;
		wildSpecies: (version: number) => Named[];
		wildEncounters: (version: number, species: number) => WildSlot[];
		/** Empty when the save can't take bred eggs (Gen 5). */
		eggSpecies: () => Named[];
		/** Return why it failed, or nothing when added. */
		addWild: (req: WildAdd) => string | undefined;
		addEgg: (req: EggAdd) => string | undefined;
	} = $props();

	const today = () => {
		const d = new Date();
		const p = (n: number) => String(n).padStart(2, '0');
		return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
	};
	const seed = () => crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
	const byName = (a: Named, b: Named) => (a.name ?? '').localeCompare(b.name ?? '');
	const name = (n: Named) => n.name ?? `#${n.id}`;

	// svelte-ignore state_referenced_locally
	let version = $state(versions[0]?.id ?? 0);
	let egg = $state(false);
	let species = $state(0);
	/** A WildSlot index, or -1 for any slot. */
	let encounter = $state(-1);
	let level: number | null = $state(null);
	let nature = $state(-1);
	let gender = $state(-1);
	let shiny = $state(false);
	let lead = $state('none');
	let ball = $state(0);
	let date = $state(today());
	let hatched = $state(false);
	let status = $state('');
	let busy = $state(false);

	const eggs = $derived(generation === 4 ? eggSpecies().toSorted(byName) : []);
	const speciesList = $derived(egg ? eggs : wildSpecies(version).toSorted(byName));
	const slots = $derived(species && !egg ? wildEncounters(version, species) : []);
	const slot = $derived(slots.find((s) => s.index === encounter));

	// A new species list or species resets what depended on it.
	$effect(() => {
		if (!speciesList.some((s) => s.id === species)) species = speciesList[0]?.id ?? 0;
	});
	$effect(() => {
		if (encounter >= 0 && !slots.some((s) => s.index === encounter)) encounter = -1;
	});

	const opt = (v: number) => (v < 0 ? undefined : v);

	function add() {
		busy = true;
		status = '';
		// Let the "Adding…" state paint before the (synchronous) search runs.
		requestAnimationFrame(() => {
			try {
				const why = egg
					? addEgg({
							version,
							species,
							nature: opt(nature),
							gender: opt(gender),
							shiny,
							hatched,
							date,
							seed: seed()
						})
					: addWild({
							version,
							species,
							form: slot?.form ?? 0,
							encounter: opt(encounter),
							level: level ?? undefined,
							nature: opt(nature),
							gender: opt(gender),
							shiny,
							lead,
							ball: ball || undefined,
							metDate: date,
							seed: seed()
						});
				status = why ?? '';
			} catch (e) {
				status = errorText(e);
			} finally {
				busy = false;
			}
		});
	}
</script>

<div class="flex flex-col gap-3 text-sm" data-testid="add-pokemon">
	{#if generation === 4}
		<div class="flex gap-2" role="group" aria-label="Kind">
			<Button size="sm" variant={egg ? 'outline' : 'secondary'} onclick={() => (egg = false)}
				>Wild</Button
			>
			<Button size="sm" variant={egg ? 'secondary' : 'outline'} onclick={() => (egg = true)}
				>Egg</Button
			>
		</div>
	{/if}

	<div class="grid grid-cols-2 gap-3">
		<div class="flex flex-col gap-1">
			<Label for="add-version">Game</Label>
			<Select.Root id="add-version" class="w-full" bind:value={version}>
				{#each versions as v (v.id)}<Select.Option value={v.id}>{name(v)}</Select.Option>{/each}
			</Select.Root>
		</div>
		<div class="flex flex-col gap-1">
			<Label for="add-species">Species</Label>
			<Select.Root id="add-species" class="w-full" bind:value={species}>
				{#each speciesList as s (s.id)}<Select.Option value={s.id}>{name(s)}</Select.Option>{/each}
			</Select.Root>
		</div>
	</div>
	<p class="text-xs text-muted-foreground">
		{speciesList.length}
		{egg ? 'species can be bred here.' : 'species have wild slots in this game.'}
	</p>

	{#if !egg}
		<div class="flex flex-col gap-1">
			<Label for="add-slot">Encounter</Label>
			<Select.Root id="add-slot" class="w-full" bind:value={encounter}>
				<Select.Option value={-1}>Any slot</Select.Option>
				{#each slots as s (s.index)}
					<Select.Option value={s.index}>
						{name(s.location)} · {s.kind} · Lv. {s.levelMin === s.levelMax
							? s.levelMin
							: `${s.levelMin}–${s.levelMax}`}
					</Select.Option>
				{/each}
			</Select.Root>
		</div>
		<div class="grid grid-cols-2 gap-3">
			<div class="flex flex-col gap-1">
				<Label for="add-level">Level</Label>
				<Input
					id="add-level"
					type="number"
					min={slot?.levelMin ?? 1}
					max={slot?.levelMax ?? 100}
					placeholder="Any"
					bind:value={level}
				/>
			</div>
			<div class="flex flex-col gap-1">
				<Label for="add-ball">Ball</Label>
				<Select.Root id="add-ball" class="w-full" bind:value={ball}>
					<Select.Option value={0}>Any</Select.Option>
					{#each choices.wildBalls as b (b.id)}<Select.Option value={b.id}>{name(b)}</Select.Option
						>{/each}
				</Select.Root>
			</div>
		</div>
	{/if}

	<div class="grid grid-cols-2 gap-3">
		<div class="flex flex-col gap-1">
			<Label for="add-nature">Nature</Label>
			<Select.Root id="add-nature" class="w-full" bind:value={nature}>
				<Select.Option value={-1}>Any</Select.Option>
				{#each choices.natures as n, i (i)}<Select.Option value={i}>{n}</Select.Option>{/each}
			</Select.Root>
		</div>
		<div class="flex flex-col gap-1">
			<Label for="add-gender">Gender</Label>
			<Select.Root id="add-gender" class="w-full" bind:value={gender}>
				<Select.Option value={-1}>Any</Select.Option>
				<Select.Option value={0}>Male</Select.Option>
				<Select.Option value={1}>Female</Select.Option>
			</Select.Root>
		</div>
	</div>

	{#if !egg && generation === 4}
		<div class="flex flex-col gap-1">
			<Label for="add-lead">Lead Pokémon</Label>
			<Select.Root id="add-lead" class="w-full" bind:value={lead}>
				<Select.Option value="none">None</Select.Option>
				<Select.Option value="synchronize">Synchronize (keeps the nature)</Select.Option>
				<Select.Option value="cuteCharm">Cute Charm (keeps the gender)</Select.Option>
				<Select.Option value="radar">Poké Radar chain (shiny)</Select.Option>
			</Select.Root>
		</div>
	{/if}

	<div class="flex flex-wrap items-end gap-4">
		<div class="flex flex-col gap-1">
			<Label for="add-date">{egg ? 'Egg date' : 'Met date'}</Label>
			<Input id="add-date" type="date" min="2000-01-01" max="2099-12-31" bind:value={date} />
		</div>
		<label class="flex h-8 items-center gap-2">
			<input type="checkbox" class="accent-primary" bind:checked={shiny} /> Shiny
		</label>
		{#if egg}
			<label class="flex h-8 items-center gap-2">
				<input type="checkbox" class="accent-primary" bind:checked={hatched} /> Hatched
			</label>
		{/if}
	</div>

	<div class="flex flex-wrap items-center gap-3">
		<Button disabled={!species || busy} onclick={add}>{busy ? 'Adding…' : 'Add'}</Button>
		<span class="text-destructive" role="status" data-testid="add-status">{status}</span>
	</div>
	<p class="text-xs text-muted-foreground">
		Every Pokémon is generated the way the game would, then checked for legality before it's
		written.
	</p>
</div>
