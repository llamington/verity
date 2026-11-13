import { describe, expect, it } from 'vitest';
import { needsConsent } from './analytics.svelte.js';

const trace = (loc: string) => `fl=1\nh=verity.llamington.workers.dev\nloc=${loc}\ntls=TLSv1.3\n`;

describe('needsConsent', () => {
	it('asks in the EEA, the UK and Switzerland', () => {
		for (const loc of ['DE', 'FR', 'NO', 'GB', 'CH']) expect(needsConsent(trace(loc))).toBe(true);
	});
	it("doesn't ask elsewhere", () => {
		for (const loc of ['US', 'NZ', 'JP', 'BR']) expect(needsConsent(trace(loc))).toBe(false);
	});
	it('asks when the country is unknown', () => {
		expect(needsConsent(undefined)).toBe(true);
		expect(needsConsent('fl=1\n')).toBe(true);
		expect(needsConsent(trace('XX1'))).toBe(true);
	});
});
