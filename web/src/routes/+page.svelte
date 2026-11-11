<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';

	const FEATURES = [
		{
			title: 'Legality checks',
			text: 'Every Pokémon is checked against the rules PKHeX uses: its origin, moves, IVs and PID, met data and ribbons. The report says which check failed and why.'
		},
		{
			title: 'An editor that stays legal',
			text: 'Change levels, moves, IVs and EVs, held items, met data, nicknames and ribbons with a live legality preview. On Gen 4 saves it finds the closest legal IVs for you.'
		},
		{
			title: 'Add legal Pokémon',
			text: 'Generate wild encounters (with Synchronize, Cute Charm and the Poké Radar), Gen 4 eggs and hatched eggs the way the games create them.'
		},
		{
			title: 'Mystery Gifts',
			text: 'Deliver Wonder Cards and other event gifts as the games do: the card goes in the album and the deliveryman hands the gift over in-game.'
		},
		{
			title: 'Bag',
			text: 'Add items and change counts in every pouch. Event items such as the Member Card stay with Mystery Gift, which also unlocks their events.'
		},
		{
			title: 'Memory Link and keys',
			text: 'Link a Black or White save into Black 2 or White 2, receive keys from another save, or enable every key.'
		}
	];
</script>

<svelte:head>
	<title>verity: Gen 4 and 5 save editor</title>
	<meta
		name="description"
		content="A legality-aware save editor for Pokémon Diamond, Pearl, Platinum, HeartGold, SoulSilver, Black, White, Black 2 and White 2. Runs in your browser; your save never leaves your device."
	/>
</svelte:head>

<main class="mx-auto flex max-w-3xl flex-col gap-10 px-4 py-16">
	<section class="flex flex-col gap-6">
		<h1 class="text-3xl font-semibold tracking-tight">verity</h1>
		<p class="text-lg text-muted-foreground">
			A legality-aware save editor for the Gen 4 and Gen 5 games: Diamond, Pearl, Platinum,
			HeartGold, SoulSilver, Black, White, Black 2 and White 2.
		</p>
		<p class="text-muted-foreground">
			It runs entirely in your browser. Your save file never leaves your device.
		</p>
		<div><Button href={resolve('/app')}>Open a save</Button></div>
	</section>

	<section aria-labelledby="features" class="flex flex-col gap-4">
		<h2 id="features" class="text-xl font-semibold">What it does</h2>
		<ul class="grid gap-4 sm:grid-cols-2">
			{#each FEATURES as f (f.title)}
				<li class="flex flex-col gap-1 rounded-lg border p-4">
					<h3 class="font-medium">{f.title}</h3>
					<p class="text-sm text-muted-foreground">{f.text}</p>
				</li>
			{/each}
		</ul>
	</section>

	<section aria-labelledby="saves" class="flex flex-col gap-2 text-sm text-muted-foreground">
		<h2 id="saves" class="text-xl font-semibold text-foreground">Your saves</h2>
		<p>
			Open a raw save (.sav) or a DeSmuME save (.dsv). In Chrome and Edge, verity can write the
			edited save back to the same file after checking it reads back intact; elsewhere it downloads
			the edited save. Either way you can download the original first, as a backup.
		</p>
	</section>

	<footer class="flex flex-col gap-1 border-t pt-6 text-xs text-muted-foreground">
		<p>
			Legality rules and game data come from
			<a class="underline" href="https://github.com/kwsch/PKHeX">PKHeX</a> (GPLv3). Sprites load
			from the <a class="underline" href="https://github.com/PokeAPI/sprites">PokeAPI sprites</a>
			repository. verity is free software under the GPLv3.
		</p>
		<p>
			verity is a fan project, not affiliated with or endorsed by Nintendo, Game Freak or The
			Pokémon Company. Pokémon and its names are trademarks of their respective owners.
		</p>
	</footer>
</main>
