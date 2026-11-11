<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import * as Select from '#lib/components/ui/native-select/index.js';
	import LegalityReport from '#lib/components/LegalityReport.svelte';
	import type { Choices, EditRequest, IvFix, Named, Preview, Summary } from '#lib/verity.js';

	let {
		summary,
		choices,
		preview,
		apply,
		closestIvs
	}: {
		summary: Summary;
		choices: Choices;
		/** Throws when the edit can't be expressed (a bad nickname, say). */
		preview: (edit: EditRequest) => Preview;
		/** Returns why it was refused, or nothing when written. */
		apply: (edit: EditRequest, legalMode: boolean) => string | undefined;
		/** Gen 4 wild catches only; absent otherwise. */
		closestIvs?: (ivs: number[]) => IvFix;
	} = $props();

	// The save stores stats as HP, Atk, Def, Spe, SpA, SpD; the form shows the games' order.
	const ORDER = [0, 1, 2, 4, 5, 3];
	const LABELS = ['HP', 'Atk', 'Def', 'SpA', 'SpD', 'Spe'];
	const CONTEST = ['Cool', 'Beauty', 'Cute', 'Smart', 'Tough', 'Sheen'];

	// The form starts from the Pokémon as saved; the parent re-creates this component after an
	// apply, so `summary` is read once here.
	// svelte-ignore state_referenced_locally
	const orig = summary;
	const origNick = orig.mon.nicknamed ? orig.mon.nickname : '';
	let nickname = $state(origNick);
	let resetNickname = $state(false);
	let heldItem = $state(orig.heldItem.id);
	let level = $state(orig.mon.level ?? 1);
	let friendship = $state(orig.friendship);
	let ivs = $state([...orig.ivs]);
	let evs = $state([...orig.evs]);
	let pid: number | undefined = $state();
	let moves = $state(orig.moves.map((m) => m.id));
	let ball = $state(orig.ball.id);
	let metLocation = $state(orig.metLocation.id);
	let metLevel = $state(orig.metLevel);
	let metDate = $state(orig.metDate);
	let contest = $state([...orig.contest]);
	let ribbons = $state([...orig.ribbons]);
	let legalMode = $state(true);
	let status = $state('');
	let ivNote = $state('');

	const same = (a: number[], b: number[]) => a.every((v, i) => v === b[i]);
	const clamp = (v: number, lo: number, hi: number) =>
		Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : lo;

	/** Only what changed, like the DS editor's field mask. */
	const request: EditRequest = $derived.by(() => {
		const r: EditRequest = {};
		if (resetNickname) r.resetNickname = true;
		else if (nickname !== origNick && nickname !== '') r.nickname = nickname;
		if (heldItem !== orig.heldItem.id) r.heldItem = heldItem;
		if (level !== orig.mon.level) r.level = clamp(level, 1, 100);
		if (friendship !== orig.friendship) r.friendship = clamp(friendship, 0, 255);
		if (!same(ivs, orig.ivs)) r.ivs = ivs.map((v) => clamp(v, 0, 31));
		if (pid !== undefined) r.pid = pid;
		if (!same(evs, orig.evs)) r.evs = evs.map((v) => clamp(v, 0, 255));
		if (
			!same(
				moves,
				orig.moves.map((m) => m.id)
			)
		)
			r.moves = moves;
		if (ball !== orig.ball.id) r.ball = ball;
		if (
			metLocation !== orig.metLocation.id ||
			metLevel !== orig.metLevel ||
			metDate !== orig.metDate
		)
			r.met = { location: metLocation, level: clamp(metLevel, 0, 100), date: metDate };
		if (!same(contest, orig.contest)) r.contest = contest.map((v) => clamp(v, 0, 255));
		if (ribbons.some((v, i) => v !== orig.ribbons[i])) r.ribbons = ribbons;
		return r;
	});
	const edited = $derived(Object.keys(request).length > 0);

	const result: { preview?: Preview; error?: string } = $derived.by(() => {
		try {
			return { preview: preview(request) };
		} catch (e) {
			return { error: e instanceof Error ? e.message : String(e) };
		}
	});

	function setIv(i: number, v: number) {
		ivs[i] = v;
		pid = undefined; // a hand-set spread no longer goes with the proposed PID
		ivNote = '';
	}

	function findClosest() {
		if (!closestIvs) return;
		try {
			const fix = closestIvs(ivs.map((v) => clamp(v, 0, 31)));
			ivs = [...fix.ivs];
			pid = fix.pid;
			ivNote = fix.distance
				? `Closest legal spread: ${fix.distance} IV point${fix.distance === 1 ? '' : 's'} away (PID changes too).`
				: 'Already a legal spread.';
		} catch (e) {
			ivNote = e instanceof Error ? e.message : String(e);
		}
	}

	function onApply() {
		const why = apply(request, legalMode);
		status = why ? `Refused: ${why}` : '';
	}

	const name = (n: Named) => n.name ?? `#${n.id}`;
	const ensure = (list: Named[], id: number) =>
		list.some((n) => n.id === id) ? list : [{ id, name: undefined }, ...list];
	const locationList = $derived(ensure(choices.locations, metLocation));
	const itemList = $derived(ensure(choices.heldItems, heldItem));
	const ballList = $derived(ensure(choices.balls, ball));
</script>

<div class="flex flex-col gap-4 text-sm" data-testid="editor">
	<div class="flex flex-col gap-2 rounded-lg border p-3">
		<div class="flex flex-wrap items-center justify-between gap-2">
			<div class="flex items-center gap-2">
				<Switch id="legal-mode" bind:checked={legalMode} />
				<Label for="legal-mode">{legalMode ? 'Legal mode' : 'Game-safe mode'}</Label>
			</div>
			<div class="flex gap-2">
				<Button
					variant="outline"
					size="sm"
					disabled={!edited}
					onclick={() => {
						nickname = origNick;
						resetNickname = false;
						heldItem = orig.heldItem.id;
						level = orig.mon.level ?? 1;
						friendship = orig.friendship;
						ivs = [...orig.ivs];
						evs = [...orig.evs];
						pid = undefined;
						moves = orig.moves.map((m) => m.id);
						ball = orig.ball.id;
						metLocation = orig.metLocation.id;
						metLevel = orig.metLevel;
						metDate = orig.metDate;
						contest = [...orig.contest];
						ribbons = [...orig.ribbons];
						status = ivNote = '';
					}}>Undo changes</Button
				>
				<Button size="sm" disabled={!edited || !!result.error} onclick={onApply}>Apply</Button>
			</div>
		</div>
		<p class="text-xs text-muted-foreground">
			{legalMode
				? 'Refuses changes that make the Pokémon illegal or unsafe for the game.'
				: 'Refuses only changes the game can’t handle; the result may be illegal.'}
		</p>
		<div class="flex flex-wrap items-center gap-2" data-testid="game-safe">
			<span class="font-medium">Game-safe</span>
			{#if result.error}
				<Badge variant="destructive">No</Badge><span>{result.error}</span>
			{:else if result.preview?.issue}
				<Badge variant="destructive">No</Badge><span>{result.preview.issue}</span>
			{:else}
				<Badge variant="secondary">OK</Badge>
			{/if}
		</div>
		<LegalityReport legality={result.preview?.legality} />
		<p class="text-muted-foreground" role="status" data-testid="edit-status">
			{status || (edited ? 'Edited, not applied yet.' : 'No changes yet.')}
		</p>
	</div>

	<Tabs.Root value="main">
		<Tabs.List class="w-full">
			<Tabs.Trigger value="main">Main</Tabs.Trigger>
			<Tabs.Trigger value="stats">Stats</Tabs.Trigger>
			<Tabs.Trigger value="moves">Moves</Tabs.Trigger>
			<Tabs.Trigger value="met">Met</Tabs.Trigger>
			<Tabs.Trigger value="misc">Misc</Tabs.Trigger>
		</Tabs.List>

		<Tabs.Content value="main" class="flex flex-col gap-3 pt-3">
			<div class="flex flex-col gap-1">
				<Label for="nickname">Nickname</Label>
				<div class="flex gap-2">
					<Input
						id="nickname"
						bind:value={nickname}
						maxlength={choices.nicknameMax}
						placeholder={orig.mon.speciesName ?? ''}
						disabled={resetNickname}
						autocomplete="off"
					/>
					<Button
						variant="outline"
						size="sm"
						class="shrink-0"
						disabled={!orig.mon.nicknamed && !resetNickname}
						onclick={() => {
							resetNickname = !resetNickname;
							nickname = resetNickname ? '' : origNick;
						}}>{resetNickname ? 'Keep nickname' : 'Use species name'}</Button
					>
				</div>
			</div>
			<div class="flex flex-col gap-1">
				<Label for="held-item">Held item</Label>
				<Select.Root id="held-item" class="w-full" bind:value={heldItem}>
					{#each itemList as it (it.id)}
						<Select.Option value={it.id}>{it.id ? name(it) : 'None'}</Select.Option>
					{/each}
				</Select.Root>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="flex flex-col gap-1">
					<Label for="level">Level</Label>
					<Input id="level" type="number" min="1" max="100" bind:value={level} />
				</div>
				<div class="flex flex-col gap-1">
					<Label for="friendship">Friendship</Label>
					<Input id="friendship" type="number" min="0" max="255" bind:value={friendship} />
				</div>
			</div>
		</Tabs.Content>

		<Tabs.Content value="stats" class="flex flex-col gap-3 pt-3">
			<div class="grid grid-cols-[auto_repeat(6,minmax(0,1fr))] items-center gap-1 text-center">
				<span></span>
				{#each LABELS as l (l)}<span class="text-muted-foreground">{l}</span>{/each}
				<span class="pr-1 text-left text-muted-foreground">IV</span>
				{#each ORDER as i (i)}
					<Input
						type="number"
						min="0"
						max="31"
						aria-label="{LABELS[ORDER.indexOf(i)]} IV"
						class="px-1 text-center"
						value={ivs[i]}
						oninput={(e) => setIv(i, e.currentTarget.valueAsNumber)}
					/>
				{/each}
				<span class="pr-1 text-left text-muted-foreground">EV</span>
				{#each ORDER as i (i)}
					<Input
						type="number"
						min="0"
						max="255"
						aria-label="{LABELS[ORDER.indexOf(i)]} EV"
						class="px-1 text-center"
						bind:value={evs[i]}
					/>
				{/each}
			</div>
			{#if result.preview?.summary.stats}
				<p class="text-muted-foreground tabular-nums">
					Stats: {ORDER.map(
						(i, k) => `${LABELS[k]} ${result.preview?.summary.stats?.[i] ?? ''}`
					).join(' · ')}
				</p>
			{/if}
			{#if closestIvs}
				<div class="flex flex-wrap items-center gap-2">
					<Button variant="outline" size="sm" onclick={findClosest}>Closest legal IVs</Button>
					<span class="text-muted-foreground" data-testid="iv-note">{ivNote}</span>
				</div>
			{/if}
		</Tabs.Content>

		<Tabs.Content value="moves" class="flex flex-col gap-2 pt-3">
			{#each [0, 1, 2, 3] as i (i)}
				{@const after = result.preview?.summary.moves[i]}
				{@const check = result.preview?.legality.moves[i]}
				<div class="flex flex-col gap-1">
					<Select.Root aria-label="Move {i + 1}" class="w-full" bind:value={moves[i]}>
						<Select.Option value={0}>—</Select.Option>
						{#each choices.moves as m (m.id)}
							<Select.Option value={m.id}>{name(m)}</Select.Option>
						{/each}
					</Select.Root>
					<div class="flex flex-wrap items-center gap-2 text-xs">
						{#if after?.moveType}<Badge variant="outline">{name(after.moveType)}</Badge>{/if}
						{#if after?.id}<span class="text-muted-foreground">PP {after.pp}</span>{/if}
						{#if check?.how}
							<span class={check.valid ? 'text-muted-foreground' : 'text-destructive'}
								>{check.how}</span
							>
						{/if}
					</div>
				</div>
			{/each}
			<p class="text-xs text-muted-foreground">Changing moves refills PP and clears PP Ups.</p>
		</Tabs.Content>

		<Tabs.Content value="met" class="flex flex-col gap-3 pt-3">
			<div class="flex flex-col gap-1">
				<Label for="ball">Ball</Label>
				<Select.Root id="ball" class="w-full" bind:value={ball}>
					{#each ballList as b (b.id)}<Select.Option value={b.id}>{name(b)}</Select.Option>{/each}
				</Select.Root>
			</div>
			<div class="flex flex-col gap-1">
				<Label for="met-location">Met location</Label>
				<Select.Root id="met-location" class="w-full" bind:value={metLocation}>
					{#each locationList as l (l.id)}
						<Select.Option value={l.id}>{name(l)}</Select.Option>
					{/each}
				</Select.Root>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="flex flex-col gap-1">
					<Label for="met-level">Met level</Label>
					<Input id="met-level" type="number" min="0" max="100" bind:value={metLevel} />
				</div>
				<div class="flex flex-col gap-1">
					<Label for="met-date">Met date</Label>
					<Input id="met-date" type="date" min="2000-01-01" max="2099-12-31" bind:value={metDate} />
				</div>
			</div>
		</Tabs.Content>

		<Tabs.Content value="misc" class="flex flex-col gap-3 pt-3">
			<div class="grid grid-cols-3 gap-2">
				{#each CONTEST as c, i (c)}
					<div class="flex flex-col gap-1">
						<Label for="contest-{i}">{c}</Label>
						<Input id="contest-{i}" type="number" min="0" max="255" bind:value={contest[i]} />
					</div>
				{/each}
			</div>
			<fieldset class="flex flex-col gap-1">
				<legend class="mb-1 font-medium">
					Ribbons <span class="font-normal text-muted-foreground"
						>({ribbons.filter(Boolean).length})</span
					>
				</legend>
				<div class="grid grid-cols-2 gap-x-3 gap-y-1">
					{#each choices.ribbons as r, i (i)}
						<label class="flex items-center gap-2">
							<input type="checkbox" class="accent-primary" bind:checked={ribbons[i]} />
							<span class="truncate">{r}</span>
						</label>
					{/each}
				</div>
			</fieldset>
		</Tabs.Content>
	</Tabs.Root>
</div>
