<script lang="ts">
	import { resolve } from '$app/paths';
	import ShieldCheck from '@lucide/svelte/icons/shield-check';
	import SquarePen from '@lucide/svelte/icons/square-pen';
	import Egg from '@lucide/svelte/icons/egg';
	import Gift from '@lucide/svelte/icons/gift';
	import Backpack from '@lucide/svelte/icons/backpack';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import { Button } from '#lib/components/ui/button/index.js';
	import BallMark from '#lib/components/BallMark.svelte';
	import { gameGradient, typeSwatch } from '#lib/theme.js';

	const FEATURES = [
		{
			title: 'Legality checks',
			icon: ShieldCheck,
			type: 12,
			text: 'Every Pokémon is checked against the rules PKHeX uses: its origin, moves, IVs and PID, met data and ribbons. The report says which check failed and why.'
		},
		{
			title: 'An editor that stays legal',
			icon: SquarePen,
			type: 10,
			text: 'Change levels, moves, IVs and EVs, held items, met data, nicknames and ribbons with a live legality preview. On Gen 4 saves it finds the closest legal IVs for you.'
		},
		{
			title: 'Add legal Pokémon',
			icon: Egg,
			type: 11,
			text: 'Generate wild encounters (with Synchronize, Cute Charm and the Poké Radar), Gen 4 eggs and hatched eggs the way the games create them.'
		},
		{
			title: 'Mystery Gifts',
			icon: Gift,
			type: 13,
			text: 'Deliver Wonder Cards and other event gifts as the games do: the card goes in the album and the deliveryman hands the gift over in-game.'
		},
		{
			title: 'Bag',
			icon: Backpack,
			type: 9,
			text: 'Add items and change counts in every pouch. Event items such as the Member Card stay with Mystery Gift, which also unlocks their events.'
		},
		{
			title: 'Memory Link and keys',
			icon: KeyRound,
			type: 15,
			text: 'Link a Black or White save into Black 2 or White 2, receive keys from another save, or enable every key.'
		}
	];
	const GAMES = [
		{ id: 'dp', name: 'Diamond · Pearl' },
		{ id: 'pt', name: 'Platinum' },
		{ id: 'hgss', name: 'HeartGold · SoulSilver' },
		{ id: 'bw', name: 'Black · White' },
		{ id: 'b2w2', name: 'Black 2 · White 2' }
	];
</script>

<svelte:head>
	<title>verity: Gen 4 and 5 save editor</title>
	<meta
		name="description"
		content="A legality-aware save editor for Pokémon Diamond, Pearl, Platinum, HeartGold, SoulSilver, Black, White, Black 2 and White 2. Runs in your browser; your save never leaves your device."
	/>
</svelte:head>

<header class="relative overflow-hidden bg-primary text-primary-foreground">
	<BallMark
		size={420}
		outline
		class="pointer-events-none absolute -top-24 -right-28 text-white opacity-15"
	/>
	<div class="relative mx-auto flex max-w-3xl flex-col gap-6 px-4 pt-16 pb-14">
		<h1 class="flex items-center gap-3 font-pixel text-5xl font-bold tracking-wide drop-shadow">
			<BallMark size={52} class="animate-[wobble_3s_ease-in-out_infinite]" />verity
		</h1>
		<p class="max-w-xl text-lg font-semibold">
			A legality-aware save editor for the Gen 4 and Gen 5 games: Diamond, Pearl, Platinum,
			HeartGold, SoulSilver, Black, White, Black 2 and White 2.
		</p>
		<p class="opacity-90">
			It runs entirely in your browser. Your save file never leaves your device.
		</p>
		<div>
			<Button
				href={resolve('/app')}
				size="lg"
				variant="secondary"
				class="font-pixel text-base shadow-[0_4px_0_var(--ball-band)] transition-transform hover:-translate-y-0.5 active:translate-y-0.5 active:shadow-none"
				>Open a save <span aria-hidden="true">▶</span></Button
			>
		</div>
	</div>
	<div class="h-3 bg-[var(--ball-band)]"></div>
</header>

<main class="mx-auto flex max-w-3xl flex-col gap-12 px-4 py-12">
	<ul class="flex flex-wrap gap-2" aria-label="Supported games">
		{#each GAMES as g (g.id)}
			<li
				class="rounded-full px-3 py-1 text-sm font-bold text-white shadow-sm"
				style:background-image={gameGradient(g.id)}
			>
				{g.name}
			</li>
		{/each}
	</ul>

	<section aria-labelledby="features" class="flex flex-col gap-4">
		<h2 id="features" class="font-pixel text-2xl font-bold">What it does</h2>
		<ul class="grid gap-4 sm:grid-cols-2">
			{#each FEATURES as f (f.title)}
				{@const sw = typeSwatch(f.type)}
				<li
					class="flex gap-3 rounded-2xl border-2 bg-card p-4 shadow-sm transition-transform hover:-translate-y-0.5"
				>
					<span
						class="flex size-10 shrink-0 items-center justify-center rounded-full shadow-[inset_0_-3px_0_rgb(0_0_0/0.15)]"
						style:background-color={sw.bg}
						style:color={sw.fg}
					>
						<f.icon size={20} aria-hidden="true" />
					</span>
					<div class="flex flex-col gap-1">
						<h3 class="font-bold">{f.title}</h3>
						<p class="text-sm text-muted-foreground">{f.text}</p>
					</div>
				</li>
			{/each}
		</ul>
	</section>

	<section aria-labelledby="saves" class="flex flex-col gap-2 text-sm text-muted-foreground">
		<h2 id="saves" class="font-pixel text-2xl font-bold text-foreground">Your saves</h2>
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
