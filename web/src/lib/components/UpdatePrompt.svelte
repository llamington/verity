<script lang="ts">
	import { onMount } from 'svelte';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import { Button } from '#lib/components/ui/button/index.js';
	import { doc } from '#lib/editor.svelte.js';

	// A new release installs in the background (src/service-worker) and waits; this offers to switch
	// to it. Switching reloads the page, so it's the user's choice (unsaved edits ask first).
	const HOURLY = 60 * 60 * 1000;

	let waiting: ServiceWorker | undefined = $state.raw();
	let dismissed = $state(false);
	let reloading = false;

	onMount(() => {
		if (!('serviceWorker' in navigator)) return;
		const sw = navigator.serviceWorker;
		let reg: ServiceWorkerRegistration | undefined;

		function track(worker: ServiceWorker | null) {
			if (!worker) return;
			const check = () => {
				// With no controller this is the first install, which takes over by itself.
				if (worker.state === 'installed' && sw.controller) {
					waiting = worker;
					dismissed = false;
				}
			};
			check();
			worker.addEventListener('statechange', check);
		}
		const lookForUpdate = () => {
			if (document.visibilityState === 'visible') reg?.update().catch(() => undefined);
		};
		const onControllerChange = () => {
			if (reloading) location.reload();
		};

		void sw.ready.then((r) => {
			reg = r;
			track(r.waiting);
			track(r.installing);
			r.addEventListener('updatefound', () => {
				track(r.installing);
			});
		});
		sw.addEventListener('controllerchange', onControllerChange);
		document.addEventListener('visibilitychange', lookForUpdate);
		const timer = setInterval(lookForUpdate, HOURLY);
		return () => {
			sw.removeEventListener('controllerchange', onControllerChange);
			document.removeEventListener('visibilitychange', lookForUpdate);
			clearInterval(timer);
		};
	});

	function reload() {
		if (!waiting) return;
		reloading = true;
		waiting.postMessage('skip-waiting');
	}
</script>

{#if waiting && !dismissed}
	<div
		role="status"
		class="fixed right-4 bottom-4 left-4 z-50 flex flex-wrap items-center gap-3 rounded-2xl border-2 border-primary/40 bg-card p-3 text-sm shadow-lg sm:left-auto sm:max-w-sm"
		data-testid="update-prompt"
	>
		<RefreshCw size={18} class="shrink-0 text-primary" aria-hidden="true" />
		<p class="min-w-0 flex-1">
			A new version of Verity is ready.{doc.dirty ? ' Save your changes first.' : ''}
		</p>
		<div class="flex gap-2">
			<Button size="sm" variant="ghost" onclick={() => (dismissed = true)}>Later</Button>
			<Button size="sm" onclick={reload}>Reload</Button>
		</div>
	</div>
{/if}
