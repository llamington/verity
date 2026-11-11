<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PokemonSummary from './PokemonSummary.svelte';

	const n = (id: number, name?: string) => ({ id, name });
	const summary = {
		mon: {
			species: 157,
			speciesName: 'Typhlosion',
			nickname: 'Cayenne',
			nicknamed: true,
			level: 100,
			form: 0,
			egg: false,
			shiny: false,
			checksumOk: true
		},
		pid: 0x9626da57,
		exp: 1059860,
		nature: n(16, 'Jolly'),
		ability: n(66, 'Blaze'),
		hiddenAbility: false,
		gender: 0,
		heldItem: n(0, 'None'),
		ball: n(4, 'Poké Ball'),
		types: [n(9, 'Fire')],
		moves: [
			{ id: 332, name: 'Aerial Ace', pp: 32, ppUps: 3, moveType: n(2, 'Flying') },
			{ id: 89, name: 'Earthquake', pp: 16, ppUps: 3, moveType: n(4, 'Ground') },
			{ id: 29, name: 'Headbutt', pp: 15, ppUps: 0, moveType: n(0, 'Normal') },
			{ id: 53, name: 'Flamethrower', pp: 24, ppUps: 3, moveType: n(9, 'Fire') }
		],
		ivs: [11, 3, 8, 23, 26, 29],
		evs: [252, 252, 0, 6, 0, 0],
		stats: [340, 239, 169, 251, 224, 204],
		friendship: 255,
		ot: 'RoC',
		otFemale: false,
		tid: 41896,
		sid: 35319,
		language: 'English',
		origin: 'HeartGold',
		metLocation: n(126, 'New Bark Town'),
		metLevel: 5,
		metDate: '2016-05-08',
		eggLocation: n(0),
		eggDate: undefined,
		fateful: false,
		contest: [0, 0, 0, 0, 0, 0],
		markings: [true, false, true, false, false, false],
		ribbonCount: 2
	};

	const { Story } = defineMeta({
		title: 'Summary/PokemonSummary',
		component: PokemonSummary,
		args: { summary }
	});
</script>

<Story name="Default" />
<Story
	name="With move checks"
	args={{
		legality: {
			status: 'illegal',
			reason: undefined,
			origin: undefined,
			checks: [],
			moves: [
				{ how: 'TM/HM', valid: true },
				{ how: 'TM/HM', valid: true },
				{ how: "can't be learnt", valid: false },
				{ how: 'level up (42)', valid: true }
			]
		}
	}}
/>
<Story
	name="Egg"
	args={{
		summary: {
			...summary,
			mon: { ...summary.mon, egg: true, level: 1 },
			eggLocation: n(2000, 'Day-Care Couple'),
			eggDate: '2016-05-08'
		}
	}}
/>
<Story name="No game data" args={{ summary: { ...summary, stats: undefined, types: [] } }} />
