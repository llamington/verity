<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import UnovaLink from './UnovaLink.svelte';
	import type { UnovaPlan } from '#lib/verity.js';

	const base: UnovaPlan = {
		mode: 'memoryLink',
		ready: true,
		replaces: false,
		status: '',
		flashback: true,
		keys: [],
		trainer: undefined
	};
	const unusable: UnovaPlan = {
		...base,
		mode: 'unusable',
		ready: false,
		status: 'Needs a valid B/W or B2/W2 save'
	};
	// Stand-ins for the WASM calls, picked by the file's first byte.
	const plans: Record<number, UnovaPlan> = {
		1: { ...base, trainer: 'Hilda' },
		2: {
			...base,
			trainer: 'Hilda',
			replaces: true,
			status: "Replaces another player's link",
			flashback: false
		},
		3: { ...base, mode: 'keys', keys: ['Challenge Mode', 'City swap'], trainer: 'Nate' },
		4: unusable
	};
	const planOf = (other: Uint8Array): UnovaPlan => plans[other[0] ?? 4] ?? unusable;
	const file = (first: number, name: string) => new File([new Uint8Array([first, 0, 0])], name);

	const { Story } = defineMeta({
		title: 'Unova/UnovaLink',
		component: UnovaLink,
		args: {
			plan: fn(planOf),
			link: fn(() => undefined),
			enablePlan: fn((): UnovaPlan => ({
				...base,
				mode: 'enableAll',
				keys: ['Easy Mode', 'Iron Chamber'],
				trainer: undefined
			})),
			enableAll: fn(() => undefined)
		}
	});
</script>

<Story name="Default" />

<Story
	name="Memory Link from a Black save"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.upload(canvas.getByLabelText('Partner save'), file(1, 'black.sav'));
		const plan = await canvas.findByTestId('unova-plan');
		await expect(plan).toHaveTextContent('Memory Link');
		await expect(plan).toHaveTextContent('Trainer Hilda');
		await expect(plan).toHaveTextContent('Unlocks a new flashback.');
		await userEvent.click(canvas.getByRole('button', { name: 'Link' }));
		await expect(args.link).toHaveBeenCalledOnce();
		await expect(canvas.getByTestId('unova-message')).toHaveTextContent('Linked with black.sav');
		await expect(canvas.queryByTestId('unova-plan')).toBeNull();
	}}
/>

<Story
	name="Replacing another player's link is ready, with a note"
	play={async ({ canvas, userEvent }) => {
		await userEvent.upload(canvas.getByLabelText('Partner save'), file(2, 'white.sav'));
		const plan = await canvas.findByTestId('unova-plan');
		await expect(plan).toHaveTextContent("Replaces another player's link.");
		await expect(canvas.getByRole('button', { name: 'Link' })).toBeEnabled();
	}}
/>

<Story
	name="Keys from a Black 2 save"
	play={async ({ canvas, userEvent }) => {
		await userEvent.upload(canvas.getByLabelText('Partner save'), file(3, 'b2.sav'));
		const keys = await canvas.findByRole('list', { name: 'Keys' });
		await expect(keys).toHaveTextContent('Challenge Mode');
		await expect(keys).toHaveTextContent('City swap');
	}}
/>

<Story
	name="Another file can't be linked"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.upload(canvas.getByLabelText('Partner save'), file(4, 'platinum.sav'));
		const plan = await canvas.findByTestId('unova-plan');
		await expect(plan).toHaveTextContent('Needs a valid B/W or B2/W2 save');
		await expect(canvas.getByRole('button', { name: 'Link' })).toBeDisabled();
		await expect(args.link).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Enable all keys asks first"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('button', { name: 'Enable all keys…' }));
		await expect(args.enableAll).not.toHaveBeenCalled();
		await expect(canvas.getByRole('list', { name: 'Keys' })).toHaveTextContent('Iron Chamber');
		await expect(canvas.getByTestId('unova-plan')).toHaveTextContent("can't be sent");
		await userEvent.click(canvas.getByRole('button', { name: 'Enable these keys' }));
		await expect(args.enableAll).toHaveBeenCalledOnce();
		await expect(canvas.getByTestId('unova-message')).toHaveTextContent('Keys enabled');
	}}
/>
