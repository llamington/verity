<script lang="ts">
	import { asset, resolve } from '$app/paths';
	import ShieldCheck from '@lucide/svelte/icons/shield-check';
	import SquarePen from '@lucide/svelte/icons/square-pen';
	import Egg from '@lucide/svelte/icons/egg';
	import Gift from '@lucide/svelte/icons/gift';
	import Backpack from '@lucide/svelte/icons/backpack';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import Download from '@lucide/svelte/icons/download';
	import { Button } from '#lib/components/ui/button/index.js';
	import { goto } from '$app/navigation';
	import OpenSaveButton from '#lib/components/OpenSaveButton.svelte';
	import BallMark from '#lib/components/BallMark.svelte';
	import Logo from '#lib/components/Logo.svelte';
	import { onMount } from 'svelte';
	import { analyticsAvailable, consent } from '#lib/analytics.svelte.js';
	import { DS_DOWNLOAD, RELEASES, REPO, SITE } from '#lib/site.js';
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
	// Captured by scripts/screenshots.ts at 2x; sizes are in CSS pixels.
	const SHOT_CLASS = 'h-auto w-full rounded-2xl border-2 bg-card shadow-sm';
	const SOURCES = [
		{
			title: 'Emulator',
			text: 'melonDS and most emulators keep a .sav beside the game or in a saves folder; DeSmuME and DraStic use .dsv. Close the game first, or the emulator may overwrite your edits when it exits.'
		},
		{
			title: 'TWiLight Menu++ or a flashcart',
			text: 'The .sav is on the SD card, in a saves folder next to the game or beside the game itself.'
		},
		{
			title: 'Game cartridge',
			text: 'Copy the save off the cartridge with Checkpoint on a 3DS or GodMode9i on a DSi, and restore the edited save with the same tool.'
		}
	];
	const DS_STEPS = [
		'Download verity.nds and copy it to your SD card.',
		'Open it from TWiLight Menu++ or a flashcart, or load it in melonDS.',
		'Pick a save from the card. On first boot Verity builds its fonts and sprites from a Gen 4 or 5 game it finds there, so no Nintendo assets ship with it.'
	];
	const GAMES = [
		{ id: 'dp', name: 'Diamond · Pearl' },
		{ id: 'pt', name: 'Platinum' },
		{ id: 'hgss', name: 'HeartGold · SoulSilver' },
		{ id: 'bw', name: 'Black · White' },
		{ id: 'b2w2', name: 'Black 2 · White 2' }
	];
	// The footer offers to change the analytics choice where analytics can run.
	let showAnalyticsLink = $state(false);
	onMount(() => {
		showAnalyticsLink = analyticsAvailable();
	});
	const TITLE = 'Verity: Pokémon Gen 4 and 5 save editor';
	const DESCRIPTION =
		'A legality-aware save editor for Pokémon Diamond, Pearl, Platinum, HeartGold, SoulSilver, Black, White, Black 2 and White 2. A PKHeX alternative that runs in your browser, even offline, or on the DS itself; your save never leaves your device.';
	const JSON_LD = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': 'WebApplication',
		name: 'Verity',
		url: `${SITE}/`,
		description: DESCRIPTION,
		applicationCategory: 'UtilitiesApplication',
		operatingSystem: 'Any (runs in the browser), Nintendo DS',
		browserRequirements: 'Requires JavaScript and WebAssembly',
		downloadUrl: DS_DOWNLOAD,
		isAccessibleForFree: true,
		license: 'https://www.gnu.org/licenses/gpl-3.0.html',
		offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
		image: `${SITE}/screenshots/social.png`,
		sameAs: [REPO]
	});
	// The closing tag is escaped so neither this script block nor the parser sees one; `<` is escaped so
	// the JSON can never end the tag either.
	const JSON_LD_TAG = `<script type="application/ld+json">${JSON_LD.replaceAll('<', '\\u003c')}<\u002fscript>`;
	/** WebP sources for a screenshot (or DS photo) stored at 2x; `width` is its CSS width. */
	function shot(name: 'summary' | 'edit' | 'gifts' | 'ds-summary' | 'ds-gifts', width: number) {
		const x1 = asset(`screenshots/${name}-1x.webp`);
		const x2 = asset(`screenshots/${name}.webp`);
		return { src: x2, srcset: `${x1} ${String(width)}w, ${x2} ${String(width * 2)}w` };
	}
	// Half the 736px content column (less the gap) from sm up, else the screen less its gutters.
	const HALF = '(min-width: 640px) 360px, calc(100vw - 32px)';
	const FULL = '(min-width: 768px) 736px, calc(100vw - 32px)';
</script>

<svelte:head>
	<title>{TITLE}</title>
	<meta name="description" content={DESCRIPTION} />
	<link rel="canonical" href="{SITE}/" />
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="Verity" />
	<meta property="og:url" content="{SITE}/" />
	<meta property="og:title" content={TITLE} />
	<meta property="og:description" content={DESCRIPTION} />
	<meta property="og:image" content="{SITE}/screenshots/social.png" />
	<meta property="og:image:width" content="2048" />
	<meta property="og:image:height" content="904" />
	<meta
		property="og:image:alt"
		content="Verity's Mystery Gift list for a Platinum save, each gift with a Deliver button."
	/>
	<meta name="twitter:card" content="summary_large_image" />
	<!-- JSON_LD_TAG is built from our own constants, so {@html} is safe here. -->
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html JSON_LD_TAG}
</svelte:head>

<header class="relative overflow-hidden bg-primary text-primary-foreground">
	<BallMark
		size={420}
		outline
		class="pointer-events-none absolute -top-24 -right-28 text-white opacity-15"
	/>
	<div class="relative mx-auto flex max-w-3xl flex-col gap-6 px-4 pt-16 pb-14">
		<h1 class="flex items-center gap-3 font-pixel text-5xl font-bold tracking-wide drop-shadow">
			<Logo size={48} tile class="animate-[wobble_3s_ease-in-out_infinite]" />verity
		</h1>
		<p class="max-w-xl text-lg font-semibold">
			A legality-aware save editor for the Gen 4 and Gen 5 games: Diamond, Pearl, Platinum,
			HeartGold, SoulSilver, Black, White, Black 2 and White 2.
		</p>
		<p>
			An alternative to PKHeX that uses the same legality rules and runs entirely in your browser,
			even offline once it has loaded, or on the DS itself. Your save file never leaves your device.
		</p>
		<div class="flex flex-wrap items-center gap-x-6 gap-y-3">
			<!-- Picks the save here, then opens the editor with it loaded. -->
			<OpenSaveButton
				size="lg"
				variant="secondary"
				testid="landing-file"
				onopened={() => void goto(resolve('/app'))}
				class="font-pixel text-base shadow-[0_4px_0_var(--ball-band)] transition-transform hover:-translate-y-0.5 active:translate-y-0.5 active:shadow-none"
				>Open a save <span aria-hidden="true">▶</span></OpenSaveButton
			>
			<a class="font-semibold underline underline-offset-4" href="#ds">Run it on a DS</a>
			<a
				class="font-semibold underline underline-offset-4"
				href="https://github.com/llamington/verity">View source on GitHub</a
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

	<section aria-labelledby="look" class="flex flex-col gap-4">
		<h2 id="look" class="font-pixel text-2xl font-bold">A look inside</h2>
		<div class="grid gap-4 sm:grid-cols-2 sm:items-start">
			<figure class="flex flex-col gap-2">
				<img
					{...shot('summary', 384)}
					sizes={HALF}
					width="384"
					height="700"
					fetchpriority="high"
					class={SHOT_CLASS}
					alt="A Floatzel's summary: a Legal verdict with its origin, then nature, ability, items, moves and stats with IVs and EVs."
				/>
				<figcaption class="text-sm text-muted-foreground">
					Every Pokémon's summary leads with its legality verdict.
				</figcaption>
			</figure>
			<figure class="flex flex-col gap-2">
				<img
					{...shot('edit', 384)}
					sizes={HALF}
					width="384"
					height="610"
					loading="lazy"
					class={SHOT_CLASS}
					alt="The editor with Legal mode on, a game-safe check, the legality verdict, and fields for nickname, held item, level and friendship."
				/>
				<figcaption class="text-sm text-muted-foreground">
					In Legal mode the editor refuses changes that would make the Pokémon illegal.
				</figcaption>
			</figure>
		</div>
		<figure class="flex flex-col gap-2">
			<img
				{...shot('gifts', 1024)}
				sizes={FULL}
				width="1024"
				height="452"
				loading="lazy"
				class={SHOT_CLASS}
				alt="The Mystery Gift list for a Platinum save: searchable event gifts with their original trainer, ID and region, each with a Deliver button."
			/>
			<figcaption class="text-sm text-muted-foreground">
				Mystery Gifts for your save's game and language, ready to deliver.
			</figcaption>
		</figure>
		<p class="text-xs text-muted-foreground">
			The app shows Pokémon sprites; they're left out of these pictures.
		</p>
	</section>

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

	<section aria-labelledby="ds" class="flex scroll-mt-6 flex-col gap-4">
		<h2 id="ds" class="font-pixel text-2xl font-bold">On your DS</h2>
		<p class="text-sm text-muted-foreground">
			Verity also runs as DS homebrew, with the same legality checks and features as the browser
			version. It edits the saves on your SD card directly, checks each save reads back intact after
			writing it, and keeps your last three versions beside it as .bak1 to .bak3.
		</p>
		<div class="grid gap-4 sm:grid-cols-2 sm:items-start">
			<figure class="flex flex-col gap-2">
				<img
					{...shot('ds-summary', 360)}
					sizes={HALF}
					width="360"
					height="372"
					loading="lazy"
					class={SHOT_CLASS}
					alt="A DSi XL running Verity: a Magmar's summary with a Legal verdict on the top screen, and a box of Pokémon on the touch screen."
				/>
				<figcaption class="text-sm text-muted-foreground">
					The same legality verdict, on a real DSi XL.
				</figcaption>
			</figure>
			<figure class="flex flex-col gap-2">
				<img
					{...shot('ds-gifts', 360)}
					sizes={HALF}
					width="360"
					height="372"
					loading="lazy"
					class={SHOT_CLASS}
					alt="A DSi XL running Verity: the Mystery Gift list, with Ash's Pikachu ready to collect from the deliveryman."
				/>
				<figcaption class="text-sm text-muted-foreground">
					Mystery Gifts delivered straight to the save on your card.
				</figcaption>
			</figure>
		</div>
		<ol class="grid gap-4 sm:grid-cols-3">
			{#each DS_STEPS as step, i (step)}
				<li class="flex flex-col gap-1 rounded-2xl border-2 bg-card p-4 shadow-sm">
					<span class="font-pixel font-bold">Step {i + 1}</span>
					<p class="text-sm text-muted-foreground">{step}</p>
				</li>
			{/each}
		</ol>
		<div class="flex flex-wrap items-center gap-x-6 gap-y-3">
			<Button href={DS_DOWNLOAD} class="font-pixel"
				><Download aria-hidden="true" />Download verity.nds</Button
			>
			<a class="text-sm underline underline-offset-4" rel="external" href={RELEASES}
				>About this release</a
			>
		</div>
	</section>

	<section aria-labelledby="saves" class="flex flex-col gap-4">
		<h2 id="saves" class="font-pixel text-2xl font-bold">Getting your save</h2>
		<p class="text-sm text-muted-foreground">
			In the browser, Verity opens a raw save (.sav) or a DeSmuME save (.dsv). Where to find yours:
		</p>
		<ul class="grid gap-4 sm:grid-cols-3">
			{#each SOURCES as s (s.title)}
				<li class="flex flex-col gap-1 rounded-2xl border-2 bg-card p-4 shadow-sm">
					<h3 class="font-bold">{s.title}</h3>
					<p class="text-sm text-muted-foreground">{s.text}</p>
				</li>
			{/each}
		</ul>
		<p class="text-sm text-muted-foreground">
			In Chrome and Edge, Verity can write the edited save back to the same file after checking it
			reads back intact; elsewhere it downloads the edited save. Either way, download the original
			first as a backup.
		</p>
	</section>

	<footer class="flex flex-col gap-1 border-t pt-6 text-xs text-muted-foreground">
		<p>
			Legality rules and game data come from
			<a class="underline" href="https://github.com/kwsch/PKHeX">PKHeX</a> (GPLv3). Sprites load
			from the <a class="underline" href="https://github.com/PokeAPI/sprites">PokeAPI sprites</a>
			repository. Verity is free software under the GPLv3; its source is
			<a class="underline" href="https://github.com/llamington/verity">on GitHub</a>.
		</p>
		<p>
			Verity is a fan project, not affiliated with or endorsed by Nintendo, Game Freak or The
			Pokémon Company. Pokémon and its names are trademarks of their respective owners.
		</p>
		<p>
			Verity counts visits with Google Analytics; nothing about your save is ever sent.
			{#if showAnalyticsLink}
				<button type="button" class="underline" onclick={() => (consent.open = true)}
					>Analytics settings</button
				>
			{/if}
		</p>
	</footer>
</main>
