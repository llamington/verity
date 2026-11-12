<script lang="ts">
	import GiftList from '#lib/components/GiftList.svelte';
	import { loadGiftData, type Gift } from '#lib/verity.js';
	import { doc } from '#lib/editor.svelte.js';
	import { errorText } from '#lib/utils.js';

	let gifts: Gift[] | undefined = $state.raw();
	let error = $state('');

	// The gift databases are fetched the first time this page opens.
	$effect(() => {
		const s = doc.session;
		if (!s) return;
		gifts = undefined;
		error = '';
		loadGiftData(s)
			.then(() => {
				if (doc.session === s) gifts = s.gifts();
			})
			.catch((err: unknown) => {
				error = `Couldn't load the gift data: ${errorText(err)}`;
			});
	});

	function deliver(g: Gift, date: string): string | undefined {
		const s = doc.session;
		if (!s) return 'no save open';
		const why = s.deliverGift(g.source, g.record, date);
		if (!why) {
			doc.changed();
			// Statuses of other cards can change too (album or deliveryman full).
			gifts = s.gifts();
		}
		return why;
	}
</script>

<svelte:head>
	<title>Mystery Gifts · Verity editor</title>
</svelte:head>

{#if error}
	<p class="text-destructive" role="alert">{error}</p>
{:else if !gifts}
	<p class="text-muted-foreground">Loading the gift data…</p>
{:else}
	<GiftList {gifts} game={doc.overview?.game} {deliver} />
{/if}
