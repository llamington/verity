<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, waitFor } from 'storybook/test';
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
		addWild: fn((r: { lead: string; nature?: number | undefined }) =>
			r.lead === 'synchronize' && r.nature === undefined
				? 'Synchronize: choose a nature'
				: undefined
		),
		addEgg: fn(() => undefined)
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

<Story
	name="Sends the chosen slot, level and lead"
	play={async ({ canvas, userEvent, args }) => {
		// Species are listed by name: Bidoof first.
		await expect(canvas.getByLabelText('Species')).toHaveValue('399');
		await userEvent.selectOptions(canvas.getByLabelText('Encounter'), '2'); // Lake Verity
		await userEvent.type(canvas.getByLabelText('Level'), '3');
		await userEvent.selectOptions(canvas.getByLabelText('Lead Pokémon'), 'synchronize');
		await userEvent.click(canvas.getByRole('button', { name: 'Add' }));
		await waitFor(() =>
			expect(args.addWild).toHaveBeenLastCalledWith(
				expect.objectContaining({
					version: 12,
					species: 399,
					encounter: 2,
					level: 3,
					lead: 'synchronize',
					nature: undefined
				})
			)
		);
		await waitFor(() =>
			expect(canvas.getByTestId('add-status')).toHaveTextContent('Synchronize: choose a nature')
		);
		await userEvent.selectOptions(canvas.getByLabelText('Nature'), '3');
		await userEvent.click(canvas.getByRole('button', { name: 'Add' }));
		await waitFor(() =>
			expect(args.addWild).toHaveBeenLastCalledWith(expect.objectContaining({ nature: 3 }))
		);
	}}
/>

<Story
	name="Egg mode sends an egg request"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('button', { name: 'Egg' }));
		await expect(canvas.queryByLabelText('Encounter')).toBeNull();
		await userEvent.click(canvas.getByLabelText('Hatched'));
		await userEvent.click(canvas.getByRole('button', { name: 'Add' }));
		await waitFor(() =>
			expect(args.addEgg).toHaveBeenLastCalledWith(
				expect.objectContaining({ version: 12, species: 399, hatched: true, shiny: false })
			)
		);
		await expect(args.addWild).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Gen 5 has no eggs or leads"
	args={{ generation: 5, versions: [n(22, 'White 2'), n(23, 'Black 2')], eggSpecies: () => [] }}
	play={async ({ canvas }) => {
		await expect(canvas.queryByRole('button', { name: 'Egg' })).toBeNull();
		await expect(canvas.queryByLabelText('Lead Pokémon')).toBeNull();
		await expect(canvas.getByLabelText('Game')).toHaveValue('22');
	}}
/>
