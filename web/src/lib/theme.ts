/** Colours for the games' types, by type id (Normal 0 … Dark 16, Fairy 17). */
export interface Swatch {
	bg: string;
	fg: string;
}

const DARK_TEXT = 'oklch(0.25 0.03 265)';
const LIGHT_TEXT = 'oklch(0.99 0 0)';

const TYPES: readonly Swatch[] = [
	{ bg: 'oklch(0.72 0.05 105)', fg: DARK_TEXT }, // Normal
	{ bg: 'oklch(0.5 0.18 28)', fg: LIGHT_TEXT }, // Fighting
	{ bg: 'oklch(0.7 0.13 290)', fg: DARK_TEXT }, // Flying
	{ bg: 'oklch(0.52 0.17 325)', fg: LIGHT_TEXT }, // Poison
	{ bg: 'oklch(0.8 0.11 85)', fg: DARK_TEXT }, // Ground
	{ bg: 'oklch(0.68 0.11 100)', fg: DARK_TEXT }, // Rock
	{ bg: 'oklch(0.72 0.17 120)', fg: DARK_TEXT }, // Bug
	{ bg: 'oklch(0.48 0.1 300)', fg: LIGHT_TEXT }, // Ghost
	{ bg: 'oklch(0.78 0.03 280)', fg: DARK_TEXT }, // Steel
	{ bg: 'oklch(0.7 0.17 55)', fg: DARK_TEXT }, // Fire
	{ bg: 'oklch(0.64 0.15 260)', fg: LIGHT_TEXT }, // Water
	{ bg: 'oklch(0.74 0.17 135)', fg: DARK_TEXT }, // Grass
	{ bg: 'oklch(0.86 0.17 95)', fg: DARK_TEXT }, // Electric
	{ bg: 'oklch(0.66 0.2 5)', fg: LIGHT_TEXT }, // Psychic
	{ bg: 'oklch(0.85 0.07 195)', fg: DARK_TEXT }, // Ice
	{ bg: 'oklch(0.5 0.25 285)', fg: LIGHT_TEXT }, // Dragon
	{ bg: 'oklch(0.45 0.05 50)', fg: LIGHT_TEXT }, // Dark
	{ bg: 'oklch(0.72 0.12 345)', fg: DARK_TEXT } // Fairy
];

/** The swatch for a type id; an unknown id gets a neutral grey. */
export function typeSwatch(id: number): Swatch {
	return TYPES[id] ?? { bg: 'oklch(0.7 0.01 265)', fg: DARK_TEXT };
}

/** Each game pair's two colours, for the trainer card. */
const GAMES: Record<string, readonly [string, string]> = {
	dp: ['oklch(0.5 0.13 255)', 'oklch(0.7 0.12 350)'],
	pt: ['oklch(0.38 0.03 265)', 'oklch(0.66 0.1 75)'],
	hgss: ['oklch(0.62 0.13 75)', 'oklch(0.62 0.03 250)'],
	bw: ['oklch(0.22 0.01 265)', 'oklch(0.55 0.01 265)'],
	b2w2: ['oklch(0.28 0.06 250)', 'oklch(0.55 0.17 25)']
};

/** A CSS gradient in the colours of the save's games. */
export function gameGradient(game: string): string {
	const [a, b] = GAMES[game] ?? ['oklch(0.5 0.18 27)', 'oklch(0.4 0.1 265)'];
	return `linear-gradient(120deg, ${a}, ${b})`;
}

/** How many badges the game's region has (HeartGold and SoulSilver have two regions). */
export function badgeSlots(game: string): number {
	return game === 'hgss' ? 16 : 8;
}
