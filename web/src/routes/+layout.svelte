<script lang="ts">
	import './layout.css';
	import favicon from '#lib/assets/favicon.svg';
	import UpdatePrompt from '#lib/components/UpdatePrompt.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} type="image/svg+xml" />
	<link rel="apple-touch-icon" href="/apple-touch-icon.png" />
</svelte:head>

{@render children()}

<UpdatePrompt />
