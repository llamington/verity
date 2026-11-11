<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import LegalityReport from './LegalityReport.svelte';

	const move = (how: string, valid = true) => ({ how, valid });
	const legal = {
		status: 'legal',
		reason: undefined,
		checks: [],
		moves: [move('level up (32)'), move('TM/HM'), move('tutor'), move('')],
		origin: 'wild, Route 201'
	};

	const { Story } = defineMeta({
		title: 'Summary/LegalityReport',
		component: LegalityReport,
		args: { legality: legal }
	});
</script>

<Story name="Legal" />
<Story
	name="Illegal"
	args={{
		legality: {
			...legal,
			status: 'illegal',
			checks: ['PID', 'IVs'],
			moves: [move("can't be learnt", false), move('TM/HM'), move(''), move('')]
		}
	}}
/>
<Story
	name="Not checked"
	args={{
		legality: {
			status: 'unchecked',
			reason: 'origin not modelled (event, trade or Pokéwalker)',
			checks: [],
			moves: [],
			origin: undefined
		}
	}}
/>
<Story name="Checking" args={{ legality: undefined }} />
<Story
	name="Data failed to load"
	args={{ legality: undefined, error: "Couldn't load the legality data: data/enc_hgss.bin: 404" }}
/>
