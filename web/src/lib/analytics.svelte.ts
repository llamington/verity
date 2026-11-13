// Google Analytics page views, on the live site only (not dev, previews or tests). Visitors where
// the law asks for consent first (the EEA, the UK and Switzerland) see a banner, and GA loads
// only once they accept. Ads signals are always off. Nothing about a save is ever sent.
import { SITE } from './site.js';

export const GA_ID = 'G-8ZE23N0QC2';

const KEY = 'verity-analytics';
// EU, the rest of the EEA, the UK and Switzerland (ISO 3166 codes, as Cloudflare reports them).
const CONSENT_REGIONS = new Set(
	(
		'AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE ' +
		'IS LI NO GB CH'
	).split(' ')
);

export type Choice = 'granted' | 'denied';

/** The banner's state; `open` also reopens it from the footer to change the choice. */
export const consent = $state({ open: false });

/**
 * Whether a visitor needs to consent first, from Cloudflare's /cdn-cgi/trace body. An unknown
 * country counts as needing it.
 */
export function needsConsent(trace: string | undefined): boolean {
	const loc = trace === undefined ? undefined : /^loc=([A-Z]{2})$/m.exec(trace)?.[1];
	return loc === undefined || CONSENT_REGIONS.has(loc);
}

function stored(): Choice | undefined {
	try {
		const v = localStorage.getItem(KEY);
		return v === 'granted' || v === 'denied' ? v : undefined;
	} catch {
		return undefined;
	}
}

declare global {
	interface Window {
		dataLayer?: unknown[];
	}
}

/** gtag.js reads `arguments` objects from the data layer; plain arrays are ignored. */
const gtag: (...args: unknown[]) => void = function () {
	window.dataLayer ??= [];
	// eslint-disable-next-line prefer-rest-params -- gtag.js requires the arguments object
	window.dataLayer.push(arguments);
};

let loaded = false;

function load(): void {
	if (loaded) return;
	loaded = true;
	gtag('js', new Date());
	gtag('consent', 'default', {
		ad_storage: 'denied',
		ad_user_data: 'denied',
		ad_personalization: 'denied',
		analytics_storage: 'granted'
	});
	// SvelteKit navigations are history changes, which GA counts as page views by itself.
	gtag('config', GA_ID);
	const script = document.createElement('script');
	script.async = true;
	script.src = `https://www.googletagmanager.com/gtag/js?id=${GA_ID}`;
	document.head.append(script);
}

async function trace(): Promise<string | undefined> {
	try {
		const res = await fetch('/cdn-cgi/trace');
		return res.ok ? await res.text() : undefined;
	} catch {
		return undefined;
	}
}

/** Starts analytics, or opens the banner, on the live site. */
export async function startAnalytics(): Promise<void> {
	if (location.origin !== SITE) return;
	const choice = stored();
	if (choice === 'granted') load();
	else if (choice === undefined) {
		if (needsConsent(await trace())) consent.open = true;
		else load();
	}
}

/** Records the visitor's choice; declining after accepting stops GA and clears its cookies. */
export function choose(choice: Choice): void {
	consent.open = false;
	try {
		localStorage.setItem(KEY, choice);
	} catch {
		// Private mode: the choice holds for this page only.
	}
	if (choice === 'granted') {
		load();
		return;
	}
	if (!loaded) return;
	gtag('consent', 'update', { analytics_storage: 'denied' });
	for (const c of document.cookie.split(';')) {
		const name = c.split('=')[0]?.trim() ?? '';
		if (name.startsWith('_ga')) {
			document.cookie = `${name}=; Max-Age=0; Path=/; Domain=${location.hostname}`;
		}
	}
}

/** Whether to offer a way to change the choice (only where analytics can run). */
export function analyticsAvailable(): boolean {
	return location.origin === SITE;
}
