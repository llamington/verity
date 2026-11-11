<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PokemonEditor from './PokemonEditor.svelte';
	import { choices, summary } from './fixtures.js';
	import type { EditRequest, Preview } from '#lib/verity.js';

	const how = ['TM/HM', 'TM/HM', 'tutor', 'level up (42)'];
	// Stand-ins for the WASM calls: anything above level 99 is "illegal", Master Ball is unsafe.
	function preview(e: EditRequest): Preview {
		const illegal = (e.level ?? 0) > 99;
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
	const apply = (e: EditRequest, legal: boolean) =>
		legal && (e.level ?? 0) > 99 ? 'illegal (legal mode)' : undefined;
	const closestIvs = (ivs: number[]) => ({
		ivs: ivs.map((v) => v & ~1),
		pid: 0x12345678,
		distance: ivs.filter((v) => v & 1).length
	});

	const { Story } = defineMeta({
		title: 'Summary/PokemonEditor',
		component: PokemonEditor,
		args: { summary, choices, preview, apply, closestIvs }
	});
</script>

<Story name="Gen 4 wild catch" />
<Story name="Gen 5 (no closest IVs)" args={{ closestIvs: undefined }} />
<Story
	name="Bad nickname"
	args={{
		preview: () => {
			throw new Error('nickname: Gen 4 can’t store “€”');
		}
	}}
/>
