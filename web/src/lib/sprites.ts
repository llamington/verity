// Pokémon sprites, hotlinked at runtime from the PokeAPI sprites repository through jsDelivr
// (never bundled or re-hosted), pinned to one commit. Change SPRITES_BASE to use another host
// with the same layout. A sprite that fails to load falls back to the next candidate, then to
// the species name (see Sprite.svelte).

export const SPRITES_COMMIT = '1b3b7c0a538dd2226ae08d0d2f8955125075bbd4';
export const SPRITES_BASE = `https://cdn.jsdelivr.net/gh/PokeAPI/sprites@${SPRITES_COMMIT}/sprites/pokemon/versions`;

/** The repository's folder for each game (Black 2 and White 2 use Black and White's). */
const FOLDERS: Record<string, { path: string; maxSpecies: number; generation: 4 | 5 }> = {
	dp: { path: 'generation-iv/diamond-pearl', maxSpecies: 493, generation: 4 },
	pt: { path: 'generation-iv/platinum', maxSpecies: 493, generation: 4 },
	hgss: { path: 'generation-iv/heartgold-soulsilver', maxSpecies: 493, generation: 4 },
	bw: { path: 'generation-v/black-white', maxSpecies: 649, generation: 5 },
	b2w2: { path: 'generation-v/black-white', maxSpecies: 649, generation: 5 }
};

// prettier-ignore
const UNOWN = [
	'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
	't', 'u', 'v', 'w', 'x', 'y', 'z', 'exclamation', 'question'
];
// Arceus's forms follow the type order; Gen 4 has the ??? type (form 9), Gen 5 doesn't.
// prettier-ignore
const TYPES5 = [
	'normal',
	'fighting',
	'flying',
	'poison',
	'ground',
	'rock',
	'bug',
	'ghost',
	'steel',
	'fire',
	'water',
	'grass',
	'electric',
	'psychic',
	'ice',
	'dragon',
	'dark'
];
const TYPES4 = [...TYPES5.slice(0, 9), 'unknown', ...TYPES5.slice(9)];
/** Form names (index = the save's form number; form 0 is the plain sprite). */
const FORMS: Record<number, string[]> = {
	172: ['', 'spiky-eared'],
	386: ['', 'attack', 'defense', 'speed'],
	412: ['', 'sandy', 'trash'],
	413: ['', 'sandy', 'trash'],
	421: ['', 'sunshine'],
	422: ['', 'east'],
	423: ['', 'east'],
	479: ['', 'heat', 'wash', 'frost', 'fan', 'mow'],
	487: ['', 'origin'],
	492: ['', 'sky'],
	550: ['', 'blue-striped'],
	555: ['', 'zen'],
	585: ['', 'summer', 'autumn', 'winter'],
	586: ['', 'summer', 'autumn', 'winter'],
	641: ['', 'therian'],
	642: ['', 'therian'],
	645: ['', 'therian'],
	646: ['', 'white', 'black'],
	647: ['', 'resolute'],
	648: ['', 'pirouette'],
	649: ['', 'douse', 'shock', 'burn', 'chill']
};

// Species with a female sprite (female/<species>.png, shiny too), listed from the repository at
// SPRITES_COMMIT; asking for any other female sprite is a 404. The Gen 4 folders share one list;
// Black and White add Eevee, Unfezant, Frillish and Jellicent. No form has a female sprite.
// prettier-ignore
const FEMALE4 = [
	3, 12, 19, 20, 25, 26, 41, 42, 44, 45, 64, 65, 84, 85, 97, 111, 112, 118, 119, 123, 129, 130,
	154, 165, 166, 178, 185, 186, 190, 194, 195, 198, 202, 203, 207, 208, 212, 214, 215, 217, 221,
	224, 229, 232, 255, 256, 257, 267, 269, 272, 274, 275, 307, 308, 315, 316, 317, 322, 323, 332,
	350, 369, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 407, 415, 417, 418, 419, 424, 443,
	444, 445, 449, 450, 453, 454, 456, 457, 459, 460, 461, 464, 465, 473
];
const FEMALE: Record<4 | 5, ReadonlySet<number>> = {
	4: new Set(FEMALE4),
	5: new Set([...FEMALE4, 133, 521, 592, 593])
};

/** The repository's form suffix, or '' for the plain sprite. */
export function formSuffix(species: number, form: number, generation: 4 | 5): string {
	if (form === 0) return '';
	if (species === 201) return UNOWN[form] ?? '';
	if (species === 493) return (generation === 4 ? TYPES4 : TYPES5)[form] ?? '';
	return FORMS[species]?.[form] ?? '';
}

export interface SpriteMon {
	species: number;
	form: number;
	female: boolean;
	shiny: boolean;
	egg: boolean;
}

/** Sprite URLs to try in order, most specific first; empty when there's no sprite to show. */
export function spriteUrls(game: string, mon: SpriteMon): string[] {
	const folder = FOLDERS[game];
	if (!folder || mon.egg || mon.species < 1 || mon.species > folder.maxSpecies) return [];
	const form = formSuffix(mon.species, mon.form, folder.generation);
	const dir = `${SPRITES_BASE}/${folder.path}/${mon.shiny ? 'shiny/' : ''}`;
	const names = [String(mon.species)];
	if (form) names.unshift(`${String(mon.species)}-${form}`);
	const female = mon.female && FEMALE[folder.generation].has(mon.species);
	const urls = [
		...(female ? [`${dir}female/${String(mon.species)}.png`] : []),
		...names.map((n) => `${dir}${n}.png`)
	];
	return [...new Set(urls)];
}

const ITEMS_BASE = `https://cdn.jsdelivr.net/gh/PokeAPI/sprites@${SPRITES_COMMIT}/sprites/items`;

/**
 * An item's icon, by its English name ("King's Rock" → kings-rock, Gen 4's "NeverMeltIce" →
 * never-melt-ice). TMs and HMs share one disc. Other languages' names miss and fall back.
 */
export function itemIconUrl(name: string | undefined): string | undefined {
	if (!name) return undefined;
	const disc = /^(TM|HM)\s?\d+/i.exec(name);
	if (disc) return `${ITEMS_BASE}/${(disc[1] ?? 'tm').toLowerCase()}-normal.png`;
	const slug = name
		.replace(/([a-z])([A-Z])/g, '$1 $2')
		.normalize('NFD')
		.replace(/[̀-ͯ]/g, '')
		.toLowerCase()
		.replace(/['’.]/g, '')
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-|-$/g, '');
	return slug ? `${ITEMS_BASE}/${slug}.png` : undefined;
}
