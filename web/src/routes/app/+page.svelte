<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import {
		backupName,
		canWriteInPlace,
		download,
		pickSave,
		writeInPlace,
		type OpenedFile
	} from '#lib/files.js';
	import MonTile from '#lib/components/MonTile.svelte';
	import PokemonSummary from '#lib/components/PokemonSummary.svelte';
	import LegalityReport from '#lib/components/LegalityReport.svelte';
	import PokemonEditor from '#lib/components/PokemonEditor.svelte';
	import {
		openSave,
		loadLegalityData,
		checksOut,
		type Session,
		type Overview,
		type BoxView,
		type Summary,
		type Legality,
		type Choices,
		type EditRequest
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
	/** Where the save came from, for writing back in place (Chromium). */
	let handle: FileSystemFileHandle | undefined = $state.raw();
	/** The file as opened, kept for a backup download. */
	let original: Uint8Array | undefined = $state.raw();
	/** Edits applied since the last save. */
	let dirty = $state(false);
	/** Whether the checksums in the file as opened (or last saved) were valid. */
	let fileChecksumsOk = $state(true);
	let saveMessage = $state('');
	let saveError = $state('');
	let saving = $state(false);
	/** A file waiting for the user to confirm discarding unsaved edits. */
	let pending: OpenedFile | undefined = $state.raw();
	const inPlace = canWriteInPlace();
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
		editing = false;
		summary = s.summary(box, slot);
		legality = undefined;
		legalityError = '';
		try {
			await loadLegalityData(s);
			// Ignore a result for a Pokémon that is no longer open.
			if (session !== s || selected?.box !== box || selected.slot !== slot) return;
			legality = s.legality(box, slot);
			summary = s.summary(box, slot);
			choices ??= s.choices();
		} catch (err) {
			legalityError = `Couldn't load the legality data: ${err instanceof Error ? err.message : err}`;
		}
	}

	/** Bumped after every applied edit, so views re-read the save. */
	let version = $state(0);
	let editing = $state(false);
	let choices: Choices | undefined = $state.raw();

	const box: BoxView | undefined = $derived(
		session && overview && version >= 0 ? session.boxView(boxIndex) : undefined
	);

	function applyEdit(edit: EditRequest, legalMode: boolean): string | undefined {
		if (!session || !selected) return 'nothing open';
		const why = session.apply(selected.box, selected.slot, edit, legalMode);
		if (why) return why;
		summary = session.summary(selected.box, selected.slot);
		legality = session.legality(selected.box, selected.slot);
		overview = session.overview();
		version++;
		dirty = true;
		saveMessage = '';
		return undefined;
	}

	async function save() {
		if (!session) return;
		saving = true;
		saveError = saveMessage = '';
		try {
			const bytes = session.finalize();
			// Re-open what will be written: it must parse with valid checksums.
			if (!checksOut(bytes)) throw new Error('the finalized save fails its own checksums');
			if (handle) {
				await writeInPlace(handle, bytes);
				saveMessage = `Saved to ${fileName} and read back intact.`;
			} else {
				download(fileName, bytes);
				saveMessage = `Downloaded ${fileName}. Put it in place of your old save to use it.`;
			}
			session.markSaved();
			dirty = false;
			fileChecksumsOk = true;
			overview = session.overview();
		} catch (err) {
			saveError = err instanceof Error ? err.message : String(err);
		} finally {
			saving = false;
		}
	}

	async function load(f: OpenedFile) {
		error = '';
		try {
			const next = await openSave(f.bytes.slice());
			session?.free();
			session = next;
			overview = session.overview();
			fileName = f.name;
			handle = f.handle;
			original = f.bytes;
			dirty = false;
			fileChecksumsOk = overview.checksumsOk;
			saveMessage = saveError = '';
			boxIndex = 0;
			selected = undefined;
			choices = undefined;
		} catch (err) {
			error = `Couldn't open ${f.name}: ${err instanceof Error ? err.message : err}`;
		}
	}

	/** Opens `f`, first asking to discard unsaved edits. */
	function open(f: OpenedFile) {
		if (dirty) pending = f;
		else void load(f);
	}

	async function onPick() {
		try {
			const f = await pickSave();
			if (f) open(f);
		} catch (err) {
			error = err instanceof Error ? err.message : String(err);
		}
	}

	function onBeforeUnload(e: BeforeUnloadEvent) {
		if (dirty) e.preventDefault();
	}

	async function onFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		open({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
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
				{#if dirty}
					<Badge variant="outline">Checksums updated on save</Badge>
				{:else if fileChecksumsOk}
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
				{#if saveError}
					<span class="text-destructive">{saveError}</span>
				{:else if dirty}
					Unsaved changes.
				{:else if saveMessage}
					{saveMessage}
				{:else}
					No changes yet.
				{/if}
			</span>
			{#if original && (dirty || saveMessage)}
				<Button
					variant="outline"
					size="sm"
					onclick={() => original && download(backupName(fileName), original)}
					>Download original</Button
				>
			{/if}
			<Button size="sm" disabled={!dirty || saving} onclick={save}>
				{handle ? 'Save' : 'Save (download)'}
			</Button>
		</div>

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
						<div class="flex gap-2">
							<Button
								size="sm"
								variant={editing ? 'outline' : 'secondary'}
								onclick={() => (editing = false)}>Summary</Button
							>
							<Button
								size="sm"
								variant={editing ? 'secondary' : 'outline'}
								disabled={!choices || !legality}
								onclick={() => (editing = true)}>Edit</Button
							>
						</div>
						{#if editing && choices && session && selected}
							{@const s = session}
							{@const at = selected}
							{#key version}
								<PokemonEditor
									{summary}
									{choices}
									preview={(e) => s.preview(at.box, at.slot, e)}
									apply={applyEdit}
									closestIvs={overview.generation === 4
										? (ivs) => s.closestIvs(at.box, at.slot, new Uint8Array(ivs))
										: undefined}
								/>
							{/key}
						{:else}
							<LegalityReport {legality} error={legalityError} />
							<PokemonSummary {summary} {legality} />
						{/if}
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

<AlertDialog.Root
	open={!!pending}
	onOpenChange={(o) => {
		if (!o) pending = undefined;
	}}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>Discard unsaved changes?</AlertDialog.Title>
			<AlertDialog.Description>
				{fileName} has edits that aren't saved. Opening {pending?.name} discards them.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
			<AlertDialog.Action
				onclick={() => {
					const f = pending;
					pending = undefined;
					if (f) void load(f);
				}}>Discard and open</AlertDialog.Action
			>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
