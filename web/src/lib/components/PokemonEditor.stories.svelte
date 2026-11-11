<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, waitFor } from 'storybook/test';
	import PokemonEditor from './PokemonEditor.svelte';
	import { choices, summary } from './fixtures.js';
	import type { EditRequest, Preview } from '#lib/verity.js';

	const how = ['TM/HM', 'TM/HM', 'tutor', 'level up (42)'];
	// Stand-ins for the WASM calls: a level below 10 is "illegal", Master Ball is unsafe.
	function previewOf(e: EditRequest): Preview {
		const illegal = e.level !== undefined && e.level < 10;
		return {
			issue: e.heldItem === 1 ? 'held item not in this game' : undefined,
			legality: {
				status: illegal ? 'illegal' : 'legal',
				reason: undefined,
				checks: illegal ? ['Level'] : [],
				moves: how.map((h) => ({ how: h, valid: true })),
				origin: 'fixed encounter, New Bark Town'
			},
			summary
		};
	}
	const applyOf = (e: EditRequest, legal: boolean) =>
		legal && e.level !== undefined && e.level < 10 ? 'illegal (legal mode)' : undefined;
	const closestOf = (ivs: number[]) => ({
		ivs: ivs.map((v) => v & ~1),
		pid: 0x12345678,
		distance: ivs.filter((v) => v & 1).length
	});

	const { Story } = defineMeta({
		title: 'Summary/PokemonEditor',
		component: PokemonEditor,
		args: {
			summary,
			choices,
			preview: fn(previewOf),
			apply: fn(applyOf),
			closestIvs: fn(closestOf)
		}
	});

	/** The request the editor would send now (the latest preview call). */
	function lastRequest(preview: unknown): EditRequest {
		const calls = (preview as { mock: { calls: [EditRequest][] } }).mock.calls;
		return calls.at(-1)?.[0] ?? {};
	}
</script>

<Story name="Gen 4 wild catch" />
<Story name="Gen 5 (no closest IVs)" args={{ closestIvs: undefined }} />
<Story
	name="Bad nickname"
	args={{
		preview: fn(() => {
			throw new Error('nickname: Gen 4 can’t store “€”');
		})
	}}
	play={async ({ canvas, userEvent }) => {
		await userEvent.type(canvas.getByLabelText('Nickname'), 'x');
		await expect(canvas.getByTestId('game-safe')).toHaveTextContent('can’t store');
		await expect(canvas.getByRole('button', { name: 'Apply' })).toBeDisabled();
	}}
/>

<Story
	name="Sends only what changed, clamped"
	play={async ({ canvas, userEvent, args }) => {
		await expect(lastRequest(args.preview)).toEqual({});
		await expect(canvas.getByRole('button', { name: 'Apply' })).toBeDisabled();
		const level = canvas.getByLabelText('Level');
		await userEvent.clear(level);
		await userEvent.type(level, '150');
		await expect(lastRequest(args.preview)).toEqual({ level: 100 });
		await userEvent.clear(canvas.getByLabelText('Friendship'));
		await userEvent.type(canvas.getByLabelText('Friendship'), '70');
		await expect(lastRequest(args.preview)).toEqual({ level: 100, friendship: 70 });
		await userEvent.click(canvas.getByRole('button', { name: 'Undo changes' }));
		await expect(lastRequest(args.preview)).toEqual({});
	}}
/>

<Story
	name="Legal mode refuses, game-safe mode applies"
	play={async ({ canvas, userEvent, args }) => {
		const level = canvas.getByLabelText('Level');
		await userEvent.clear(level);
		await userEvent.type(level, '5');
		await expect(canvas.getByTestId('legality')).toHaveTextContent('Illegal');
		await userEvent.click(canvas.getByRole('button', { name: 'Apply' }));
		await expect(args.apply).toHaveBeenLastCalledWith({ level: 5 }, true);
		await expect(canvas.getByTestId('edit-status')).toHaveTextContent(
			'Refused: illegal (legal mode)'
		);
		await userEvent.click(canvas.getByRole('switch'));
		await expect(canvas.getByText('Game-safe mode')).toBeVisible();
		await userEvent.click(canvas.getByRole('button', { name: 'Apply' }));
		await expect(args.apply).toHaveBeenLastCalledWith({ level: 5 }, false);
		await expect(canvas.getByTestId('edit-status')).not.toHaveTextContent('Refused');
	}}
/>

<Story
	name="Closest IVs send the PID; a hand-set IV drops it"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('tab', { name: 'Stats' }));
		await userEvent.click(canvas.getByRole('button', { name: 'Closest legal IVs' }));
		await expect(canvas.getByTestId('iv-note')).toHaveTextContent('IV points away');
		// summary.ivs is [11, 3, 8, 23, 26, 29]; the stand-in clears the low bits.
		await expect(lastRequest(args.preview)).toEqual({
			ivs: [10, 2, 8, 22, 26, 28],
			pid: 0x12345678
		});
		const hp = canvas.getByLabelText('HP IV');
		await userEvent.clear(hp);
		await userEvent.type(hp, '31');
		await waitFor(() => expect(lastRequest(args.preview).pid).toBeUndefined());
		await expect(lastRequest(args.preview).ivs?.[0]).toBe(31);
	}}
/>

<Story
	name="Nickname set and reset"
	play={async ({ canvas, userEvent, args }) => {
		const name = canvas.getByLabelText('Nickname');
		await userEvent.clear(name);
		await userEvent.type(name, 'Blaze');
		await expect(lastRequest(args.preview)).toEqual({ nickname: 'Blaze' });
		await userEvent.click(canvas.getByRole('button', { name: 'Use species name' }));
		await expect(name).toBeDisabled();
		await expect(lastRequest(args.preview)).toEqual({ resetNickname: true });
	}}
/>

<Story
	name="Moves, met data and ribbons"
	play={async ({ canvas, userEvent, args }) => {
		await userEvent.click(canvas.getByRole('tab', { name: 'Moves' }));
		await userEvent.selectOptions(canvas.getByLabelText('Move 1'), '29'); // Headbutt
		await expect(lastRequest(args.preview).moves).toEqual([29, 89, 29, 53]);
		await userEvent.click(canvas.getByRole('tab', { name: 'Met' }));
		await userEvent.selectOptions(canvas.getByLabelText('Met location'), '127'); // Cherrygrove
		await expect(lastRequest(args.preview).met).toEqual({
			location: 127,
			level: 5,
			date: '2016-05-08'
		});
		await userEvent.click(canvas.getByRole('tab', { name: 'Misc' }));
		await userEvent.click(canvas.getByLabelText('Ribbon 2'));
		await expect(lastRequest(args.preview).ribbons?.slice(0, 3)).toEqual([true, true, false]);
	}}
/>
