<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import MonTile from '#lib/components/MonTile.svelte';
	import PokemonSummary from '#lib/components/PokemonSummary.svelte';
	import LegalityReport from '#lib/components/LegalityReport.svelte';
	import PokemonEditor from '#lib/components/PokemonEditor.svelte';
	import AddPokemon from '#lib/components/AddPokemon.svelte';
	import {
		loadLegalityData,
		type BoxView,
		type Summary,
		type Legality,
		type EditRequest,
		type WildAdd,
		type EggAdd
	} from '#lib/verity.js';
	import { doc } from '#lib/editor.svelte.js';
	import { errorText } from '#lib/utils.js';

	let boxIndex = $state(0);
	/** The open Pokémon: box index (-1 for the party) and slot. */
	let selected: { box: number; slot: number } | undefined = $state.raw();
	let summary: Summary | undefined = $state.raw();
	let legality: Legality | undefined = $state.raw();
	let legalityError = $state('');
	let editing = $state(false);
	/** The empty slot the add-Pokémon wizard fills (box -1: the party's next slot). */
	let adding: { box: number; slot: number } | undefined = $state();
	let addReady = $state(false);

	// A newly opened save starts at its first box with nothing open.
	let shownSession = doc.session;
	$effect(() => {
		if (doc.session === shownSession) return;
		shownSession = doc.session;
		boxIndex = 0;
		selected = adding = undefined;
	});

	const box: BoxView | undefined = $derived(
		doc.session && doc.version >= 0 ? doc.session.boxView(boxIndex) : undefined
	);

	async function startAdd(box: number, slot: number) {
		const s = doc.session;
		if (!s) return;
		selected = undefined;
		adding = { box, slot };
		addReady = false;
		legalityError = '';
		try {
			await loadLegalityData(s);
			if (doc.session !== s) return;
			doc.choices ??= s.choices();
			addReady = true;
		} catch (err) {
			legalityError = `Couldn't load the legality data: ${errorText(err)}`;
		}
	}

	/** After a successful add: refresh and show the new Pokémon. */
	function added(why: string | undefined): string | undefined {
		if (why || !adding) return why;
		const at = adding.box < 0 ? { box: -1, slot: doc.overview?.party.length ?? 0 } : adding;
		doc.changed();
		adding = undefined;
		void select(at.box, at.slot);
		return undefined;
	}

	async function select(box: number, slot: number) {
		const s = doc.session;
		if (!s) return;
		adding = undefined;
		const at = { box, slot };
		selected = at;
		editing = false;
		summary = s.summary(box, slot);
		legality = undefined;
		legalityError = '';
		try {
			await loadLegalityData(s);
			// Ignore a result for a Pokémon that is no longer open.
			if (doc.session !== s || selected !== at) return;
			legality = s.legality(box, slot);
			summary = s.summary(box, slot);
			doc.choices ??= s.choices();
		} catch (err) {
			legalityError = `Couldn't load the legality data: ${errorText(err)}`;
		}
	}

	function applyEdit(edit: EditRequest, legalMode: boolean): string | undefined {
		const s = doc.session;
		if (!s || !selected) return 'nothing open';
		const why = s.apply(selected.box, selected.slot, edit, legalMode);
		if (why) return why;
		summary = s.summary(selected.box, selected.slot);
		legality = s.legality(selected.box, selected.slot);
		doc.changed();
		return undefined;
	}
</script>

{#if doc.overview && box}
	{@const ov = doc.overview}
	<section class="flex flex-col gap-2">
		<h2 class="font-medium">Party</h2>
		<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-6" data-testid="party">
			{#each ov.party as mon, i (i)}
				<MonTile {mon} onselect={() => select(-1, i)} />
			{/each}
			{#if ov.party.length < 6}
				<MonTile mon={null} onadd={() => startAdd(-1, ov.party.length)} />
			{/if}
		</div>
	</section>

	<section class="flex flex-col gap-2">
		<div class="flex items-center justify-between gap-2">
			<Button
				variant="outline"
				size="sm"
				aria-label="Previous box"
				onclick={() => (boxIndex = (boxIndex + ov.boxCount - 1) % ov.boxCount)}>‹</Button
			>
			<h2 class="font-medium" data-testid="box-name">{box.name}</h2>
			<Button
				variant="outline"
				size="sm"
				aria-label="Next box"
				onclick={() => (boxIndex = (boxIndex + 1) % ov.boxCount)}>›</Button
			>
		</div>
		<div class="grid grid-cols-3 gap-2 sm:grid-cols-6">
			{#each box.slots as mon, i (i)}
				<MonTile {mon} onselect={() => select(boxIndex, i)} onadd={() => startAdd(boxIndex, i)} />
			{/each}
		</div>
	</section>

	<Sheet.Root
		open={!!selected || !!adding}
		onOpenChange={(open) => {
			if (!open) selected = adding = undefined;
		}}
	>
		<Sheet.Content class="w-full overflow-y-auto sm:max-w-md">
			{#if adding && doc.session}
				{@const s = doc.session}
				{@const at = adding}
				<Sheet.Header>
					<Sheet.Title>Add a Pokémon</Sheet.Title>
					<Sheet.Description>
						{at.box < 0 ? `Party slot ${at.slot + 1}` : `${box.name}, slot ${at.slot + 1}`}
					</Sheet.Description>
				</Sheet.Header>
				<div class="px-4 pb-6">
					{#if legalityError}
						<p class="text-destructive">{legalityError}</p>
					{:else if !addReady || !doc.choices}
						<p class="text-muted-foreground">Loading the encounter data…</p>
					{:else}
						<AddPokemon
							versions={s.wizardVersions()}
							choices={doc.choices}
							generation={ov.generation}
							wildSpecies={(v: number) => s.wildSpecies(v)}
							wildEncounters={(v: number, sp: number) => s.wildEncounters(v, sp)}
							eggSpecies={() => s.eggSpecies()}
							addWild={(r: WildAdd) => added(s.addWild(at.box, at.slot, r))}
							addEgg={(r: EggAdd) => added(s.addEgg(at.box, at.slot, r))}
						/>
					{/if}
				</div>
			{:else if summary}
				<Sheet.Header>
					<Sheet.Title>
						{summary.mon.egg ? 'Egg' : (summary.mon.speciesName ?? `#${summary.mon.species}`)}
					</Sheet.Title>
					<Sheet.Description>
						{selected && selected.box < 0
							? `Party slot ${selected.slot + 1}`
							: `${box.name}, slot ${(selected?.slot ?? 0) + 1}`}
					</Sheet.Description>
				</Sheet.Header>
				<div class="flex flex-col gap-4 px-4 pb-6">
					<div class="flex gap-2">
						<Button
							size="sm"
							variant={editing ? 'outline' : 'secondary'}
							onclick={() => (editing = false)}>Summary</Button
						>
						<Button
							size="sm"
							variant={editing ? 'secondary' : 'outline'}
							disabled={!doc.choices || !legality}
							onclick={() => (editing = true)}>Edit</Button
						>
					</div>
					{#if editing && doc.choices && doc.session && selected}
						{@const s = doc.session}
						{@const at = selected}
						{#key doc.version}
							<PokemonEditor
								{summary}
								choices={doc.choices}
								preview={(e: EditRequest) => s.preview(at.box, at.slot, e)}
								apply={applyEdit}
								closestIvs={ov.generation === 4
									? (ivs) => s.closestIvs(at.box, at.slot, new Uint8Array(ivs))
									: undefined}
							/>
						{/key}
					{:else}
						<LegalityReport {legality} error={legalityError} />
						<PokemonSummary {summary} {legality} />
					{/if}
				</div>
			{/if}
		</Sheet.Content>
	</Sheet.Root>
{/if}
