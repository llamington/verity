<script lang="ts">
	import { resolve } from '$app/paths';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/native-select/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import type { Named, Pouch } from '#lib/verity.js';

	let {
		pouches,
		addable,
		add,
		setCount,
		remove
	}: {
		pouches: Pouch[];
		/** Items that can be added to a pouch, in the game's order. */
		addable: (pouch: number) => Named[];
		/** These return why a change was refused, or nothing when made. */
		add: (pouch: number, item: number, count: number) => string | undefined;
		setCount: (pouch: number, index: number, count: number) => string | undefined;
		remove: (pouch: number, index: number) => string | undefined;
	} = $props();

	let tab = $state('0');
	const current = $derived(Number(tab));
	const pouch = $derived(pouches[current]);
	// Re-read with the pouch (a new object after every change).
	const choices = $derived(pouch ? addable(current) : []);
	let addItem = $state(0);
	let addCount = $state(1);
	let message: { ok: boolean; text: string } | undefined = $state();

	const name = (n: Named) => n.name ?? `Item #${String(n.id)}`;

	function report(why: string | undefined, done: string) {
		message = why ? { ok: false, text: why } : { ok: true, text: done };
	}

	function onCount(index: number, input: HTMLInputElement, was: number, label: string) {
		const count = Number(input.value);
		if (count === was) return;
		const why = setCount(current, index, count);
		report(why, `${label}: ${String(count)}`);
		// A refused count goes back to what the save holds.
		if (why) input.value = String(was);
	}

	function onAdd() {
		const it = choices.find((c) => c.id === addItem);
		if (!it) return;
		report(add(current, it.id, addCount), `Added ${name(it)}`);
		addItem = 0;
		addCount = 1;
	}
</script>

<div class="flex flex-col gap-3 text-sm" data-testid="bag">
	<Tabs.Root
		bind:value={tab}
		onValueChange={() => {
			message = undefined;
			addItem = 0;
			addCount = 1;
		}}
	>
		<Tabs.List class="h-auto w-full flex-wrap">
			{#each pouches as p, i (p.name)}
				<Tabs.Trigger value={String(i)}>{p.name} ({p.items.length})</Tabs.Trigger>
			{/each}
		</Tabs.List>
	</Tabs.Root>

	{#if pouch}
		<ul class="flex flex-col divide-y rounded-lg border" aria-label={pouch.name}>
			{#each pouch.items as it, i (`${String(i)}:${String(it.item.id)}`)}
				{@const label = name(it.item)}
				<li class="flex flex-wrap items-center gap-x-4 gap-y-1 p-2 pl-3" data-testid="bag-item">
					<span class="min-w-0 flex-1 font-medium">{label}</span>
					{#if it.locked}
						<Badge variant="outline">Event item</Badge>
						<span class="text-muted-foreground">×{it.count}</span>
					{:else}
						{#if pouch.maxCount > 1}
							<Input
								type="number"
								class="h-8 w-24"
								min="1"
								max={pouch.maxCount}
								value={it.count}
								aria-label={`Count of ${label}`}
								onchange={(e: Event & { currentTarget: HTMLInputElement }) => {
									onCount(i, e.currentTarget, it.count, label);
								}}
							/>
						{/if}
						<Button
							size="sm"
							variant="outline"
							aria-label={`Remove ${label}`}
							onclick={() => {
								report(remove(current, i), `Removed ${label}`);
							}}>Remove</Button
						>
					{/if}
				</li>
			{:else}
				<li class="p-3 text-muted-foreground">This pouch is empty.</li>
			{/each}
		</ul>

		<div class="flex flex-wrap items-end gap-3">
			<div class="flex min-w-48 flex-1 flex-col gap-1">
				<Label for="bag-add">Add an item</Label>
				<Select.Root id="bag-add" class="w-full" bind:value={addItem}>
					<Select.Option value={0}>
						{choices.length ? 'Choose an item' : 'Nothing more fits this pouch'}
					</Select.Option>
					{#each choices as c (c.id)}
						<Select.Option value={c.id}>{name(c)}</Select.Option>
					{/each}
				</Select.Root>
			</div>
			{#if pouch.maxCount > 1}
				<div class="flex flex-col gap-1">
					<Label for="bag-add-count">Count</Label>
					<Input
						id="bag-add-count"
						type="number"
						class="w-24"
						min="1"
						max={pouch.maxCount}
						bind:value={addCount}
					/>
				</div>
			{/if}
			<Button disabled={!addItem} onclick={onAdd}>Add</Button>
		</div>
	{/if}

	<p
		role="status"
		class={message?.ok === false ? 'text-destructive' : 'text-muted-foreground'}
		data-testid="bag-message"
	>
		{message?.text ?? ''}
	</p>
	<p class="text-xs text-muted-foreground">
		Each pouch holds only the items the game puts in it. Event items such as the Member Card or
		Liberty Pass can't be added here, because the game unlocks their events when it hands them over:
		get them from <a class="underline" href={resolve('/app/gifts')}>Mystery Gifts</a>.
	</p>
</div>
