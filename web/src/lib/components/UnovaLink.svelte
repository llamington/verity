<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { errorText } from '#lib/utils.js';
	import type { UnovaPlan } from '#lib/verity.js';

	let {
		plan,
		link,
		enablePlan,
		enableAll
	}: {
		/** What linking another save file would do. */
		plan: (other: Uint8Array) => UnovaPlan;
		/** These return why it was refused, or nothing when done. */
		link: (other: Uint8Array) => string | undefined;
		enablePlan: () => UnovaPlan;
		enableAll: () => string | undefined;
	} = $props();

	let other: { name: string; bytes: Uint8Array; plan: UnovaPlan } | undefined = $state.raw();
	let enabling: UnovaPlan | undefined = $state.raw();
	let message: { ok: boolean; text: string } | undefined = $state();

	const TITLES: Record<string, string> = {
		memoryLink: 'Memory Link',
		keys: 'Receive keys',
		enableAll: 'Enable all keys',
		unusable: 'Not a Black, White, Black 2 or White 2 save'
	};

	async function onFile(e: Event & { currentTarget: HTMLInputElement }) {
		const input = e.currentTarget;
		const f = input.files?.[0];
		// Cleared, so picking the same file again (after linking) reads it again.
		input.value = '';
		message = undefined;
		enabling = undefined;
		if (!f) return;
		try {
			const bytes = new Uint8Array(await f.arrayBuffer());
			other = { name: f.name, bytes, plan: plan(bytes) };
		} catch (err) {
			other = undefined;
			message = { ok: false, text: `Couldn't read ${f.name}: ${errorText(err)}` };
		}
	}

	function onLink() {
		if (!other) return;
		const why = link(other.bytes);
		message = why
			? { ok: false, text: why }
			: { ok: true, text: `Linked with ${other.name}. Save to keep it.` };
		other = undefined;
	}

	function onEnable() {
		const why = enableAll();
		message = why
			? { ok: false, text: why }
			: { ok: true, text: 'Keys enabled. Save to keep them.' };
		enabling = undefined;
	}
</script>

{#snippet details(p: UnovaPlan)}
	<div class="flex flex-col gap-2 rounded-lg border p-3" data-testid="unova-plan">
		<div class="flex flex-wrap items-center gap-2">
			<span class="font-medium">{TITLES[p.mode] ?? p.mode}</span>
			{#if p.ready}
				<Badge variant="outline">Ready</Badge>
			{:else}
				<Badge variant="destructive">{p.status}</Badge>
			{/if}
		</div>
		{#if p.trainer}
			<span class="text-muted-foreground">Trainer {p.trainer}</span>
		{/if}
		{#if p.ready && p.mode === 'memoryLink'}
			<span>{p.flashback ? 'Unlocks a new flashback.' : 'No new flashbacks.'}</span>
			{#if p.replaces}
				<span class="text-amber-700 dark:text-amber-400">{p.status}.</span>
			{/if}
		{:else if p.ready}
			<span
				>{p.mode === 'enableAll' ? 'Keys this will enable:' : 'Keys this save will receive:'}</span
			>
			<ul class="flex flex-wrap gap-1" aria-label="Keys">
				{#each p.keys as k (k)}
					<li><Badge variant="secondary">{k}</Badge></li>
				{/each}
			</ul>
			{#if p.mode === 'enableAll'}
				<span class="text-muted-foreground"
					>Enabled keys can't be sent to other games, as they weren't earned.</span
				>
			{/if}
		{/if}
		<span class="text-xs text-muted-foreground">Nothing changes until you link.</span>
	</div>
{/snippet}

<div class="flex flex-col gap-4 text-sm" data-testid="unova">
	<p class="text-muted-foreground">
		Pick a Black or White save for Memory Link, or a Black 2 or White 2 save to receive its keys. Or
		enable every key without a partner.
	</p>

	<div class="flex flex-col gap-1">
		<Label for="unova-file">Partner save</Label>
		<Input id="unova-file" type="file" accept=".sav,.dsv" onchange={onFile} />
	</div>
	{#if other}
		<span class="text-muted-foreground">{other.name}</span>
		{@render details(other.plan)}
		<div>
			<Button disabled={!other.plan.ready} onclick={onLink}>Link</Button>
		</div>
	{/if}

	<div class="flex flex-col gap-2 border-t pt-4">
		{#if enabling}
			{@render details(enabling)}
			<div class="flex gap-2">
				<Button disabled={!enabling.ready} onclick={onEnable}>Enable these keys</Button>
				<Button
					variant="outline"
					onclick={() => {
						enabling = undefined;
					}}>Cancel</Button
				>
			</div>
		{:else}
			<div>
				<Button
					variant="outline"
					onclick={() => {
						other = undefined;
						message = undefined;
						enabling = enablePlan();
					}}>Enable all keys…</Button
				>
			</div>
		{/if}
	</div>

	<p
		role="status"
		class={message?.ok === false ? 'text-destructive' : 'text-muted-foreground'}
		data-testid="unova-message"
	>
		{message?.text ?? ''}
	</p>
</div>
