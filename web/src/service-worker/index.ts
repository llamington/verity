// Offline support. At install, caches the site (pages, code, WASM, data blobs, Latin fonts); the
// Japanese and Korean font subsets (~23 MB) and the hotlinked sprites are cached as pages use
// them. A new release installs alongside and waits until the page asks it to take over
// (UpdatePrompt.svelte), so an open editor never changes under unsaved edits.

import { self as sw } from '$app/service-worker';
import { version } from '$app/env';
import { immutable, assets, prerendered } from '$app/manifest';

const PRECACHE = `verity-${version}`;
const RUNTIME = `verity-runtime-${version}`;
/** Sprites are pinned to a commit, so they never change; this cache outlives releases. */
const SPRITES = 'verity-sprites';
const SPRITE_HOST = 'https://cdn.jsdelivr.net/';

/** CJK subsets are fetched only for the characters on a page; legacy .woff never by browsers.
 * _headers is Cloudflare's config, never served (one missing file fails the whole install). */
const skip = (path: string) => /noto-sans-(jp|kr)|\.woff$|\/_headers$/.test(path);
const PRECACHED = [...immutable, ...assets, ...prerendered]
	// Paths are relative to the base path, which is the worker's scope.
	.map((f) => new URL(f.path, sw.registration.scope).pathname)
	.filter((p) => !skip(p));
const precached = new Set(PRECACHED);

sw.addEventListener('install', (event) => {
	event.waitUntil(caches.open(PRECACHE).then((cache) => cache.addAll(PRECACHED)));
});

sw.addEventListener('activate', (event) => {
	event.waitUntil(
		(async () => {
			for (const key of await caches.keys())
				if (key !== PRECACHE && key !== RUNTIME && key !== SPRITES) await caches.delete(key);
			// The first install controls the page that registered it, so it works offline at once.
			await sw.clients.claim();
		})()
	);
});

sw.addEventListener('message', (event) => {
	if (event.data === 'skip-waiting') void sw.skipWaiting();
});

/** From `cacheName` if there, else from the network (and kept when it succeeds). */
async function cacheFirst(request: Request, cacheName: string): Promise<Response> {
	const cache = await caches.open(cacheName);
	// Entries are keyed by versioned URLs, so a Vary header (Origin, on some servers) mustn't
	// make a module script's request miss the copy cached at install.
	const hit = await cache.match(request, { ignoreVary: true });
	if (hit) return hit;
	const response = await fetch(request);
	if (response.ok) await cache.put(request, response.clone());
	return response;
}

/** The network first, so pages stay current; the cached copy when offline. */
async function networkFirst(request: Request): Promise<Response> {
	try {
		const response = await fetch(request);
		if (response.ok) await (await caches.open(RUNTIME)).put(request, response.clone());
		return response;
	} catch (err) {
		const hit = await caches.match(request, { ignoreSearch: true, ignoreVary: true });
		if (hit) return hit;
		throw err;
	}
}

sw.addEventListener('fetch', (event) => {
	const { request } = event;
	if (request.method !== 'GET') return;
	const url = new URL(request.url);

	if (url.href.startsWith(SPRITE_HOST)) {
		event.respondWith(cacheFirst(request, SPRITES));
		return;
	}
	if (url.origin !== sw.location.origin) return;

	if (precached.has(url.pathname)) {
		event.respondWith(cacheFirst(request, PRECACHE));
	} else if (url.pathname.startsWith('/_app/immutable/')) {
		event.respondWith(cacheFirst(request, RUNTIME));
	} else {
		event.respondWith(networkFirst(request));
	}
});
