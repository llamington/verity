<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { backupName, canWriteInPlace, download, pickSave } from '#lib/files.js';
	import { doc } from '#lib/editor.svelte.js';
	import { errorText } from '#lib/utils.js';
	import BallMark from '#lib/components/BallMark.svelte';
	import { badgeSlots, gameGradient } from '#lib/theme.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const GAMES: Record<string, string> = {
		dp: 'Diamond / Pearl',
		pt: 'Platinum',
		hgss: 'HeartGold / SoulSilver',
		bw: 'Black / White',
		b2w2: 'Black 2 / White 2'
	};
	const SECTIONS = [
		{ href: resolve('/app'), label: 'Boxes' },
		{ href: resolve('/app/bag'), label: 'Bag' },
		{ href: resolve('/app/gifts'), label: 'Mystery Gifts' }
	];
	const LINK = { href: resolve('/app/link'), label: 'Memory Link' };
	// Memory Link and keys exist in Black 2 and White 2 only.
	const sections = $derived(doc.session?.hasUnovaLink() ? [...SECTIONS, LINK] : SECTIONS);
	const inPlace = canWriteInPlace();

	async function onPick() {
		try {
			const f = await pickSave();
			if (f) doc.open(f);
		} catch (err) {
			doc.error = errorText(err);
		}
	}

	async function onFile(e: Event & { currentTarget: HTMLInputElement }) {
		const input = e.currentTarget;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		doc.open({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
	}

	function onBeforeUnload(e: BeforeUnloadEvent) {
		if (doc.dirty) e.preventDefault();
	}
</script>

<svelte:window onbeforeunload={onBeforeUnload} />

<svelte:head>
	<title>verity editor</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<header class="bg-primary text-primary-foreground shadow-[inset_0_-4px_0_var(--ball-band)]">
	<div class="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-3 px-4 py-3">
		<h1 class="font-pixel text-2xl font-bold tracking-wide">
			<a href={resolve('/')} class="flex items-center gap-2"><BallMark size={28} />verity</a>
		</h1>
		<div class="flex items-center gap-2">
			{#if inPlace}
				<Button size="sm" variant="secondary" onclick={onPick}>Open a save…</Button>
				<!-- Kept for drag-and-drop style uploads and tests; saves opened here download. -->
				<input
					type="file"
					accept=".sav,.dsv,.bin"
					class="sr-only"
					tabindex="-1"
					aria-hidden="true"
					onchange={onFile}
					data-testid="file"
				/>
			{:else}
				<label class="cursor-pointer">
					<input
						type="file"
						accept=".sav,.dsv,.bin"
						class="sr-only"
						onchange={onFile}
						data-testid="file"
					/>
					<span
						class="inline-flex h-8 items-center rounded-md bg-secondary px-3 text-sm font-semibold text-secondary-foreground"
						>Open a save…</span
					>
				</label>
			{/if}
		</div>
	</div>
</header>

<main class="mx-auto flex max-w-5xl flex-col gap-5 px-4 py-6">
	{#if doc.error}
		<p class="text-destructive" role="alert">{doc.error}</p>
	{/if}

	{#if doc.overview}
		{@const ov = doc.overview}
		{@const slots = badgeSlots(ov.game)}
		<section
			aria-label="Trainer card"
			class="relative overflow-hidden rounded-2xl p-5 text-white shadow-lg ring-1 ring-black/10"
			style:background-image={gameGradient(ov.game)}
		>
			<BallMark
				size={180}
				outline
				class="pointer-events-none absolute -top-10 -right-10 text-white opacity-20"
			/>
			<div class="relative flex flex-col gap-4">
				<div class="flex flex-wrap items-end justify-between gap-2">
					<div>
						<p class="font-pixel text-xs tracking-widest uppercase opacity-80">Trainer card</p>
						<h2 class="font-pixel text-3xl font-bold drop-shadow-sm">{ov.trainer.name}</h2>
						<p class="text-sm opacity-85">{GAMES[ov.game] ?? ov.game} · {doc.fileName}</p>
					</div>
					{#if doc.dirty}
						<Badge class="bg-white/20 text-white">Checksums updated on save</Badge>
					{:else if doc.fileChecksumsOk}
						<Badge class="bg-white/20 text-white">Checksums OK</Badge>
					{:else}
						<Badge variant="destructive">Checksums bad</Badge>
					{/if}
				</div>
				<dl
					class="grid grid-cols-2 gap-2 rounded-xl bg-black/20 p-3 text-sm backdrop-blur-sm sm:grid-cols-3"
				>
					{#each [['ID No.', String(ov.trainer.tid).padStart(5, '0')], ['Money', `$${ov.trainer.money.toLocaleString()}`], ['Time', ov.trainer.played]] as [k, v] (k)}
						<div class="flex flex-col">
							<dt class="text-xs tracking-wide uppercase opacity-75">{k}</dt>
							<dd class="text-lg font-extrabold tabular-nums">{v}</dd>
						</div>
					{/each}
				</dl>
				<!-- The save stores which badges are earned, but here only how many: the first ones fill. -->
				<div class="flex flex-wrap items-center gap-x-3 gap-y-2">
					<p class="text-sm">
						<span class="text-xs tracking-wide uppercase opacity-75">Badges</span>
						<span class="ml-1 text-lg font-extrabold tabular-nums">{ov.trainer.badges}</span>
						<span class="opacity-75">/ {slots}</span>
					</p>
					<ul class="grid grid-cols-8 gap-1.5 sm:flex" aria-hidden="true">
						{#each { length: slots }, i (i)}
							{@const earned = i < ov.trainer.badges}
							<li
								title={earned ? 'Badge earned' : 'Badge not earned'}
								class="size-4 rotate-45 rounded-[3px] border-2 {earned
									? 'border-white bg-gradient-to-br from-amber-200 to-amber-500 shadow-[0_0_6px_rgb(253_230_138/0.8)]'
									: 'border-white/40 bg-black/20'}"
							></li>
						{/each}
					</ul>
				</div>
			</div>
		</section>

		<div
			class="flex flex-wrap items-center gap-2 rounded-xl border-2 border-dashed bg-card p-3 text-sm {doc.dirty
				? 'border-primary/60'
				: ''}"
			data-testid="save-bar"
		>
			<span class="mr-auto flex items-center gap-2" role="status">
				<span
					aria-hidden="true"
					class="size-2.5 rounded-full {doc.saveError
						? 'bg-destructive'
						: doc.dirty
							? 'animate-pulse bg-primary'
							: 'bg-legal'}"
				></span>
				{#if doc.saveError}
					<span class="text-destructive">{doc.saveError}</span>
				{:else if doc.dirty}
					Unsaved changes.
				{:else if doc.saveMessage}
					{doc.saveMessage}
				{:else}
					No changes yet.
				{/if}
			</span>
			{#if doc.original && (doc.dirty || doc.saveMessage)}
				<Button
					variant="outline"
					size="sm"
					onclick={() => {
						if (doc.original) download(backupName(doc.fileName), doc.original);
					}}>Download original</Button
				>
			{/if}
			<Button size="sm" disabled={!doc.dirty || doc.saving} onclick={() => doc.save()}>
				{doc.handle ? 'Save' : 'Save (download)'}
			</Button>
		</div>

		<nav
			aria-label="Sections"
			class="flex flex-wrap gap-1.5 self-start rounded-full bg-muted p-1.5"
		>
			{#each sections as s (s.href)}
				{@const current = page.url.pathname === s.href}
				<a
					href={s.href}
					aria-current={current ? 'page' : undefined}
					class="rounded-full px-4 py-1.5 text-sm font-semibold transition-colors {current
						? 'bg-primary text-primary-foreground shadow-sm'
						: 'text-muted-foreground hover:bg-card hover:text-foreground'}">{s.label}</a
				>
			{/each}
		</nav>

		{@render children()}
	{:else if !doc.error}
		<section
			class="flex flex-col items-center gap-4 rounded-2xl border-2 border-dashed bg-card px-6 py-16 text-center"
		>
			<BallMark size={72} class="animate-[wobble_2.4s_ease-in-out_infinite]" />
			<h2 class="font-pixel text-2xl font-bold">No save loaded</h2>
			<p class="max-w-md text-muted-foreground">
				Open a Gen 4 or Gen 5 save (.sav or DeSmuME .dsv). It stays on your device.
			</p>
		</section>
	{/if}
</main>

<AlertDialog.Root
	open={!!doc.pending}
	onOpenChange={(o) => {
		if (!o) doc.pending = undefined;
	}}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>Discard unsaved changes?</AlertDialog.Title>
			<AlertDialog.Description>
				{doc.fileName} has changes that aren't saved. Opening {doc.pending?.name} discards them.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
			<AlertDialog.Action
				onclick={() => {
					const f = doc.pending;
					doc.pending = undefined;
					if (f) void doc.load(f);
				}}>Discard and open</AlertDialog.Action
			>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
