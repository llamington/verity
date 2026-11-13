export { cn } from 'cn';

/** A caught value as text for the user. */
export const errorText = (err: unknown): string =>
	err instanceof Error ? err.message : String(err);

/**
 * A calendar date (YYYY-MM-DD, as the core gives it) in the reader's locale, with the month by
 * name so day and month can't be mixed up: "16 Dec 2022", "Dec 16, 2022". Unparseable input is
 * returned as is.
 */
export function formatDate(iso: string, locale?: string): string {
	const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
	if (!m) return iso;
	// Formatted in UTC so the reader's time zone can't move it to the next or previous day.
	const date = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
	return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeZone: 'UTC' }).format(date);
}

export type WithoutChild<T> = T extends { child?: unknown } ? Omit<T, 'child'> : T;
export type WithoutChildren<T> = T extends { children?: unknown } ? Omit<T, 'children'> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
