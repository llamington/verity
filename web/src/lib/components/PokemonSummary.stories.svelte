<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PokemonSummary from './PokemonSummary.svelte';
	import { n, summary } from './fixtures.js';

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
