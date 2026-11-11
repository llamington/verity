<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import MonTile from './MonTile.svelte';

	const base = {
		species: 25,
		speciesName: 'Pikachu',
		nickname: 'PIKACHU',
		nicknamed: false,
		level: 42,
		form: 0,
		female: false,
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
<Story
	name="Empty slot that adds"
	args={{ mon: null, onadd: fn() }}
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('button', { name: 'Add a Pokémon here' }));
		await expect(args.onadd).toHaveBeenCalledOnce();
	}}
/>
<Story
	name="Selectable"
	args={{ onselect: fn() }}
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('button', { name: /Pikachu/ }));
		await expect(args.onselect).toHaveBeenCalledOnce();
	}}
/>
