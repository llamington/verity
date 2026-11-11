<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import BagEditor from './BagEditor.svelte';
	import type { Named, Pouch } from '#lib/verity.js';

	const n = (id: number, name: string): Named => ({ id, name });
	const pouches: Pouch[] = [
		{
			name: 'Items',
			maxCount: 999,
			items: [
				{ item: n(80, 'Sun Stone'), count: 2, locked: false },
				{ item: n(81, 'Moon Stone'), count: 1, locked: false }
			]
		},
		{
			name: 'Key Items',
			maxCount: 1,
			items: [
				{ item: n(450, 'Bike'), count: 1, locked: false },
				{ item: n(454, 'Member Card'), count: 1, locked: true }
			]
		},
		{ name: 'Berries', maxCount: 999, items: [] }
	];
	const addableOf = (pouch: number): Named[] =>
		pouch === 0 ? [n(82, 'Fire Stone'), n(83, 'Thunder Stone')] : [];

	const { Story } = defineMeta({
		title: 'Bag/BagEditor',
		component: BagEditor,
		args: {
			pouches,
			addable: fn(addableOf),
			add: fn(() => undefined),
			setCount: fn((_p: number, _i: number, count: number) =>
				count > 999 ? 'That count is outside what the pouch allows.' : undefined
			),
			remove: fn(() => undefined)
		}
	});
</script>

<Story name="Default" />

<Story
	name="Adds an item with a count"
	play={async ({ canvas, userEvent, args }) => {
		const add = canvas.getByRole('button', { name: 'Add' });
		await expect(add).toBeDisabled();
		await userEvent.selectOptions(canvas.getByLabelText('Add an item'), '83');
		const count = canvas.getByLabelText('Count');
		await userEvent.clear(count);
		await userEvent.type(count, '5');
		await userEvent.click(add);
		await expect(args.add).toHaveBeenLastCalledWith(0, 83, 5);
		await expect(canvas.getByTestId('bag-message')).toHaveTextContent('Added Thunder Stone');
	}}
/>

<Story
	name="Changes a count, and puts a refused one back"
	play={async ({ canvas, userEvent, args }) => {
		const count = canvas.getByLabelText('Count of Sun Stone');
		await userEvent.clear(count);
		await userEvent.type(count, '40');
		await userEvent.tab();
		await expect(args.setCount).toHaveBeenLastCalledWith(0, 0, 40);
		await expect(canvas.getByTestId('bag-message')).toHaveTextContent('Sun Stone: 40');

		await userEvent.clear(count);
		await userEvent.type(count, '5000');
		await userEvent.tab();
		await expect(canvas.getByTestId('bag-message')).toHaveTextContent('outside what the pouch');
		await expect(count).toHaveValue(2);
	}}
/>

<Story
	name="Removes an item"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('button', { name: 'Remove Moon Stone' }));
		await expect(args.remove).toHaveBeenLastCalledWith(0, 1);
		await expect(canvas.getByTestId('bag-message')).toHaveTextContent('Removed Moon Stone');
	}}
/>

<Story
	name="Event items are read-only; key items have no count"
	play={async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole('tab', { name: 'Key Items (2)' }));
		const rows = canvas.getAllByTestId('bag-item');
		await expect(rows[1]).toHaveTextContent('Event item');
		await expect(canvas.queryByRole('button', { name: 'Remove Member Card' })).toBeNull();
		await expect(canvas.getByRole('button', { name: 'Remove Bike' })).toBeVisible();
		await expect(canvas.queryByLabelText('Count of Bike')).toBeNull();
		await expect(canvas.getByLabelText('Add an item')).toHaveTextContent(
			'Nothing more fits this pouch'
		);
		await expect(canvas.getByRole('link', { name: 'Mystery Gifts' })).toBeVisible();
	}}
/>

<Story
	name="An empty pouch"
	play={async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole('tab', { name: 'Berries (0)' }));
		await expect(canvas.getByText('This pouch is empty.')).toBeVisible();
	}}
/>
