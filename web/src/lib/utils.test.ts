import { describe, expect, it } from 'vitest';
import { formatDate } from './utils.js';

describe('formatDate', () => {
	it('names the month in the reader’s locale', () => {
		expect(formatDate('2022-12-16', 'en-GB')).toBe('16 Dec 2022');
		expect(formatDate('2022-12-16', 'en-US')).toBe('Dec 16, 2022');
	});

	it('keeps the calendar day whatever the time zone', () => {
		expect(formatDate('2016-01-01', 'en-GB')).toBe('1 Jan 2016');
		expect(formatDate('2016-12-31', 'en-GB')).toBe('31 Dec 2016');
	});

	it('returns text it cannot parse unchanged', () => {
		expect(formatDate('', 'en-GB')).toBe('');
		expect(formatDate('2016-5-8', 'en-GB')).toBe('2016-5-8');
	});
});
