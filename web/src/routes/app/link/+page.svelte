<script lang="ts">
	import UnovaLink from '#lib/components/UnovaLink.svelte';
	import { doc } from '#lib/editor.svelte.js';
	import type { Session, UnovaPlan } from '#lib/verity.js';

	function session(): Session {
		if (!doc.session) throw new Error('No save is open.');
		return doc.session;
	}

	/** Runs a change on the open save; refusals come back as text. */
	function change(edit: (s: Session) => string | undefined): string | undefined {
		const why = edit(session());
		if (!why) doc.changed();
		return why;
	}

	// The game seeds the link's random values from its frame counter.
	const seed = () => crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;

	const plan = (other: Uint8Array): UnovaPlan => session().unovaPlan(other);
	const link = (other: Uint8Array) => change((s) => s.unovaLink(other, seed()));
	const enablePlan = (): UnovaPlan => session().unovaEnablePlan();
	const enableAll = () => change((s) => s.unovaEnableAll());
</script>

<svelte:head>
	<title>Memory Link · Verity editor</title>
</svelte:head>

{#if doc.session?.hasUnovaLink()}
	<!-- A newly opened save starts with nothing picked. -->
	{#key doc.session}
		<UnovaLink {plan} {link} {enablePlan} {enableAll} />
	{/key}
{:else}
	<p class="text-muted-foreground">Memory Link and keys are for Black 2 and White 2 saves.</p>
{/if}
