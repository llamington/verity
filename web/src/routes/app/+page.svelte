<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import MonTile from '#lib/components/MonTile.svelte';
	import PokemonSummary from '#lib/components/PokemonSummary.svelte';
	import LegalityReport from '#lib/components/LegalityReport.svelte';
	import {
		openSave,
		loadLegalityData,
		type Session,
		type Overview,
		type BoxView,
		type Summary,
		type Legality
	} from '#lib/verity.js';

	const GAMES: Record<string, string> = {
		dp: 'Diamond / Pearl',
		pt: 'Platinum',
		hgss: 'HeartGold / SoulSilver',
		bw: 'Black / White',
		b2w2: 'Black 2 / White 2'
	};

	let session: Session | undefined = $state.raw();
	let overview: Overview | undefined = $state.raw();
	let fileName = $state('');
	let error = $state('');
	let boxIndex = $state(0);

	/** The open Pokémon: box index (-1 for the party) and slot. */
	let selected: { box: number; slot: number } | undefined = $state();
	let summary: Summary | undefined = $state.raw();
	let legality: Legality | undefined = $state.raw();
	let legalityError = $state('');

	async function select(box: number, slot: number) {
		if (!session) return;
		const s = session;
		selected = { box, slot };
		summary = s.summary(box, slot);
		legality = undefined;
		legalityError = '';
		try {
			await loadLegalityData(s);
			// Ignore a result for a Pokémon that is no longer open.
			if (session !== s || selected?.box !== box || selected.slot !== slot) return;
			legality = s.legality(box, slot);
			summary = s.summary(box, slot);
		} catch (err) {
			legalityError = `Couldn't load the legality data: ${err instanceof Error ? err.message : err}`;
		}
	}

	const box: BoxView | undefined = $derived(
		session && overview ? session.boxView(boxIndex) : undefined
	);

	async function onFile(e: Event) {
		const file = (e.currentTarget as HTMLInputElement).files?.[0];
		if (!file) return;
		error = '';
		try {
			session?.free();
			session = await openSave(new Uint8Array(await file.arrayBuffer()));
			overview = session.overview();
			fileName = file.name;
			boxIndex = 0;
			selected = undefined;
		} catch (err) {
			session = overview = undefined;
			error = `Couldn't open ${file.name}: ${err instanceof Error ? err.message : err}`;
		}
	}
</script>

<svelte:head>
	<title>verity editor</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<main class="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6">
	<header class="flex flex-wrap items-center justify-between gap-3">
		<h1 class="text-xl font-semibold"><a href={resolve('/')}>verity</a></h1>
		<label class="cursor-pointer">
			<input
				type="file"
				accept=".sav,.dsv,.bin"
				class="sr-only"
				onchange={onFile}
				data-testid="file"
			/>
			<span
				class="inline-flex h-8 items-center rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground"
				>Open a save…</span
			>
		</label>
	</header>

	{#if error}
		<p class="text-destructive" role="alert">{error}</p>
	{/if}

	{#if overview && box}
		<Card.Root>
			<Card.Header>
				<Card.Title>{overview.trainer.name}</Card.Title>
				<Card.Description>
					{GAMES[overview.game] ?? overview.game} · {fileName}
				</Card.Description>
			</Card.Header>
			<Card.Content class="flex flex-wrap gap-x-6 gap-y-1 text-sm">
				<span>ID {String(overview.trainer.tid).padStart(5, '0')}</span>
				<span>Money ${overview.trainer.money.toLocaleString()}</span>
				<span>Badges {overview.trainer.badges}</span>
				<span>Played {overview.trainer.played}</span>
				{#if overview.checksumsOk}
					<Badge variant="secondary">Checksums OK</Badge>
				{:else}
					<Badge variant="destructive">Checksums bad</Badge>
				{/if}
			</Card.Content>
		</Card.Root>

		<section class="flex flex-col gap-2">
			<h2 class="font-medium">Party</h2>
			<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-6" data-testid="party">
				{#each overview.party as mon, i (i)}
					<MonTile {mon} onselect={() => select(-1, i)} />
				{/each}
			</div>
		</section>

		<section class="flex flex-col gap-2">
			<div class="flex items-center justify-between gap-2">
				<Button
					variant="outline"
					size="sm"
					aria-label="Previous box"
					onclick={() => (boxIndex = (boxIndex + overview!.boxCount - 1) % overview!.boxCount)}
					>‹</Button
				>
				<h2 class="font-medium" data-testid="box-name">{box.name}</h2>
				<Button
					variant="outline"
					size="sm"
					aria-label="Next box"
					onclick={() => (boxIndex = (boxIndex + 1) % overview!.boxCount)}>›</Button
				>
			</div>
			<div class="grid grid-cols-3 gap-2 sm:grid-cols-6">
				{#each box.slots as mon, i (i)}
					<MonTile {mon} onselect={() => select(boxIndex, i)} />
				{/each}
			</div>
		</section>

		<Sheet.Root
			open={!!selected}
			onOpenChange={(open) => {
				if (!open) selected = undefined;
			}}
		>
			<Sheet.Content class="w-full overflow-y-auto sm:max-w-md">
				{#if summary}
					<Sheet.Header>
						<Sheet.Title>
							{summary.mon.egg ? 'Egg' : (summary.mon.speciesName ?? `#${summary.mon.species}`)}
						</Sheet.Title>
						<Sheet.Description>
							{selected && selected.box < 0
								? `Party slot ${selected.slot + 1}`
								: `${box.name}, slot ${(selected?.slot ?? 0) + 1}`}
						</Sheet.Description>
					</Sheet.Header>
					<div class="flex flex-col gap-4 px-4 pb-6">
						<LegalityReport {legality} error={legalityError} />
						<PokemonSummary {summary} {legality} />
					</div>
				{/if}
			</Sheet.Content>
		</Sheet.Root>
	{:else if !error}
		<p class="text-muted-foreground">
			Open a Gen 4 or Gen 5 save (.sav or DeSmuME .dsv). It stays on your device.
		</p>
	{/if}
</main>
