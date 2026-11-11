<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
</script>

<svelte:head>
	<title>verity: Gen 4 and 5 save editor</title>
	<meta
		name="description"
		content="A legality-aware save editor for Pokémon Diamond, Pearl, Platinum, HeartGold, SoulSilver, Black, White, Black 2 and White 2. Runs in your browser; your save never leaves your device."
	/>
</svelte:head>

<main class="mx-auto flex max-w-2xl flex-col gap-6 px-4 py-16">
	<h1 class="text-3xl font-semibold tracking-tight">verity</h1>
	<p class="text-lg text-muted-foreground">
		A legality-aware save editor for the Gen 4 and Gen 5 games: Diamond, Pearl, Platinum, HeartGold,
		SoulSilver, Black, White, Black 2 and White 2.
	</p>
	<p class="text-muted-foreground">
		It runs entirely in your browser. Your save file never leaves your device.
	</p>
	<div><Button href={resolve('/app')}>Open a save</Button></div>
</main>
