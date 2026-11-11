<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { backupName, canWriteInPlace, download, pickSave } from '#lib/files.js';
	import { doc } from '#lib/editor.svelte.js';
	import { errorText } from '#lib/utils.js';
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

<main class="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6">
	<header class="flex flex-wrap items-center justify-between gap-3">
		<h1 class="text-xl font-semibold"><a href={resolve('/')}>verity</a></h1>
		<div class="flex items-center gap-2">
			{#if inPlace}
				<Button size="sm" onclick={onPick}>Open a save…</Button>
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
						class="inline-flex h-8 items-center rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground"
						>Open a save…</span
					>
				</label>
			{/if}
		</div>
	</header>

	{#if doc.error}
		<p class="text-destructive" role="alert">{doc.error}</p>
	{/if}

	{#if doc.overview}
		{@const ov = doc.overview}
		<Card.Root>
			<Card.Header>
				<Card.Title>{ov.trainer.name}</Card.Title>
				<Card.Description>
					{GAMES[ov.game] ?? ov.game} · {doc.fileName}
				</Card.Description>
			</Card.Header>
			<Card.Content class="flex flex-wrap gap-x-6 gap-y-1 text-sm">
				<span>ID {String(ov.trainer.tid).padStart(5, '0')}</span>
				<span>Money ${ov.trainer.money.toLocaleString()}</span>
				<span>Badges {ov.trainer.badges}</span>
				<span>Played {ov.trainer.played}</span>
				{#if doc.dirty}
					<Badge variant="outline">Checksums updated on save</Badge>
				{:else if doc.fileChecksumsOk}
					<Badge variant="secondary">Checksums OK</Badge>
				{:else}
					<Badge variant="destructive">Checksums bad</Badge>
				{/if}
			</Card.Content>
		</Card.Root>

		<div
			class="flex flex-wrap items-center gap-2 rounded-lg border p-3 text-sm"
			data-testid="save-bar"
		>
			<span class="mr-auto" role="status">
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

		<nav aria-label="Sections" class="flex gap-1 border-b">
			{#each sections as s (s.href)}
				{@const current = page.url.pathname === s.href}
				<a
					href={s.href}
					aria-current={current ? 'page' : undefined}
					class="-mb-px border-b-2 px-3 py-2 text-sm {current
						? 'border-primary font-medium'
						: 'border-transparent text-muted-foreground hover:text-foreground'}">{s.label}</a
				>
			{/each}
		</nav>

		{@render children()}
	{:else if !doc.error}
		<p class="text-muted-foreground">
			Open a Gen 4 or Gen 5 save (.sav or DeSmuME .dsv). It stays on your device.
		</p>
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
