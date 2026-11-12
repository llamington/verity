<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fireEvent } from 'storybook/test';
	import Sprite from './Sprite.svelte';
	import { SPRITES_BASE } from '#lib/sprites.js';

	const mon = { species: 422, form: 1, female: true, shiny: false, egg: false };

	const { Story } = defineMeta({
		title: 'Pokemon/Sprite',
		component: Sprite,
		args: { game: 'pt', mon, name: 'Shellos', size: 96 }
	});
</script>

<Story name="Default" />

<Story
	name="Falls back through the candidates, then to the name"
	play={async ({ canvas }) => {
		const dir = `${SPRITES_BASE}/generation-iv/platinum/`;
		const img = () => canvas.getByTestId('sprite');
		// Shellos has no female sprite, so the form sprite comes first.
		await expect(img()).toHaveAttribute('src', `${dir}422-east.png`);
		await expect(img()).toHaveAttribute('alt', 'Shellos');
		// Each failure tries the next candidate.
		for (const next of ['422.png']) {
			await fireEvent.error(img());
			await expect(img()).toHaveAttribute('src', dir + next);
		}
		await fireEvent.error(img());
		await expect(canvas.queryByTestId('sprite')).toBeNull();
		await expect(canvas.getByTestId('sprite-fallback')).toHaveAttribute('title', 'Shellos');
	}}
/>

<Story
	name="Eggs and other games show the generic icon"
	args={{ game: 'rs', name: 'Pikachu' }}
	play={async ({ canvas }) => {
		await expect(canvas.queryByTestId('sprite')).toBeNull();
		await expect(canvas.getByTestId('sprite-fallback')).toHaveTextContent('Pikachu');
	}}
/>
