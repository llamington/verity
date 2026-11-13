<script lang="ts">
	import { onMount } from 'svelte';
	import ChartColumn from '@lucide/svelte/icons/chart-column';
	import { Button } from '#lib/components/ui/button/index.js';
	import { choose, consent, startAnalytics } from '#lib/analytics.svelte.js';

	onMount(() => {
		void startAnalytics();
	});
</script>

{#if consent.open}
	<div
		role="dialog"
		aria-label="Analytics cookies"
		class="fixed right-4 bottom-4 left-4 z-50 flex flex-wrap items-center gap-3 rounded-2xl border-2 border-primary/40 bg-card p-3 text-sm shadow-lg sm:left-auto sm:max-w-sm"
		data-testid="consent-banner"
	>
		<ChartColumn size={18} class="shrink-0 text-primary" aria-hidden="true" />
		<p class="min-w-0 flex-1">
			Can Verity count your visit with Google Analytics cookies? Nothing about your save is sent.
		</p>
		<div class="flex gap-2">
			<Button
				size="sm"
				variant="ghost"
				onclick={() => {
					choose('denied');
				}}>Decline</Button
			>
			<Button
				size="sm"
				onclick={() => {
					choose('granted');
				}}>Allow</Button
			>
		</div>
	</div>
{/if}
