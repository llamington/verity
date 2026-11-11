<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import AddPokemon from './AddPokemon.svelte';
	import { choices, n } from './fixtures.js';
	import type { WildSlot } from '#lib/verity.js';

	const species = [n(399, 'Bidoof'), n(396, 'Starly'), n(403, 'Shinx')];
	const slot = (index: number, place: string, kind: string): WildSlot => ({
		index,
		location: n(index, place),
		form: 0,
		levelMin: 2,
		levelMax: 4,
		kind,
		slotNumber: 0,
		canRadar: kind === 'Grass'
	});
	const slots = [slot(1, 'Route 201', 'Grass'), slot(2, 'Lake Verity', 'Surf')];
	// Stand-ins for the WASM calls: Synchronize without a nature is refused, as in the app.
	const args = {
		versions: [n(12, 'Platinum')],
		choices,
		generation: 4,
		wildSpecies: () => species,
		wildEncounters: () => slots,
		eggSpecies: () => species,
		addWild: (r: { lead: string; nature?: number | undefined }) =>
			r.lead === 'synchronize' && r.nature === undefined
				? 'Synchronize: choose a nature'
				: undefined,
		addEgg: () => undefined
	};

	const { Story } = defineMeta({
		title: 'Box/AddPokemon',
		component: AddPokemon,
		args
	});
</script>

<Story name="Gen 4" />
<Story
	name="Gen 5"
	args={{ generation: 5, versions: [n(22, 'White 2'), n(23, 'Black 2')], eggSpecies: () => [] }}
/>
<Story name="Generator refuses" args={{ addWild: () => 'No legal frame found (try again)' }} />
