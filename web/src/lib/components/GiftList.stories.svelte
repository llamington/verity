<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import GiftList from './GiftList.svelte';
	import { gifts } from './fixtures.js';
	import type { Gift } from '#lib/verity.js';

	// Stand-in for the WASM call: the Member Card is refused, everything else delivered.
	const deliverOf = (g: Gift) => (g.record === 3 ? 'Deliveryman has 8 gifts waiting' : undefined);

	const { Story } = defineMeta({
		title: 'Gifts/GiftList',
		component: GiftList,
		args: { gifts, deliver: fn(deliverOf) }
	});
</script>

<Story name="Default" />
<Story name="Nothing offered" args={{ gifts: [] }} />

<Story
	name="Search and ready-only filters"
	play={async ({ canvas, userEvent }) => {
		const rows = () => canvas.queryAllByTestId('gift');
		await expect(rows()).toHaveLength(4);
		await expect(canvas.getByRole('status')).toHaveTextContent('4 gifts');
		await expect(canvas.getByRole('status')).toHaveTextContent('3 ready');
		// Releases of one event are told apart by OT and ID.
		await userEvent.type(canvas.getByLabelText('Search'), '41212');
		await expect(rows()).toHaveLength(1);
		await expect(rows()[0]).toHaveTextContent('OT Game');
		await userEvent.clear(canvas.getByLabelText('Search'));
		await userEvent.click(canvas.getByLabelText('Ready only'));
		await expect(rows()).toHaveLength(3);
		await expect(canvas.queryByText('Already received')).toBeNull();
	}}
/>

<Story
	name="Delivers with the chosen date"
	play={async ({ canvas, userEvent, args }) => {
		const date = canvas.getByLabelText('Received on');
		await userEvent.clear(date);
		await userEvent.type(date, '2026-01-02');
		const button = canvas.getByRole('button', {
			name: 'Deliver Shiny Pikachu (Pikachu, OT Nintendo, ID 00007)'
		});
		await userEvent.click(button);
		await expect(args.deliver).toHaveBeenLastCalledWith(gifts[0], '2026-01-02');
		await expect(canvas.getByText('Delivered')).toBeVisible();
		await expect(button).toBeDisabled();
		// A received card can't be delivered again.
		await expect(canvas.getByRole('button', { name: /Manaphy Egg/ })).toBeDisabled();
	}}
/>

<Story
	name="Shows why a delivery was refused"
	play={async ({ canvas, userEvent }) => {
		const button = canvas.getByRole('button', { name: /Member Card/ });
		await userEvent.click(button);
		await expect(canvas.getByText('Deliveryman has 8 gifts waiting')).toBeVisible();
		await expect(button).toBeEnabled();
	}}
/>

<Story
	name="A delivered card stays listed under Ready only"
	args={{
		// As the page does: after a delivery the list is re-read and the card isn't ready.
		deliver: fn(() => undefined)
	}}
	play={async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByLabelText('Ready only'));
		await userEvent.click(canvas.getByRole('button', { name: /Member Card/ }));
		await expect(canvas.getByText('Delivered')).toBeVisible();
		await expect(canvas.queryAllByTestId('gift')).toHaveLength(3);
	}}
/>
