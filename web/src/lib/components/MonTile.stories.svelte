<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MonTile from './MonTile.svelte';

	const base = {
		species: 25,
		speciesName: 'Pikachu',
		nickname: 'PIKACHU',
		nicknamed: false,
		level: 42,
		form: 0,
		egg: false,
		shiny: false,
		checksumOk: true
	};

	const { Story } = defineMeta({
		title: 'Box/MonTile',
		component: MonTile,
		args: { mon: base }
	});
</script>

<Story name="Default" />
<Story name="Nicknamed" args={{ mon: { ...base, nickname: 'Sparky', nicknamed: true } }} />
<Story name="Shiny" args={{ mon: { ...base, shiny: true } }} />
<Story name="Egg" args={{ mon: { ...base, egg: true, nickname: 'Egg' } }} />
<Story name="Bad checksum" args={{ mon: { ...base, checksumOk: false } }} />
<Story name="No data yet" args={{ mon: { ...base, speciesName: undefined, level: undefined } }} />
<Story name="Empty slot" args={{ mon: null }} />
