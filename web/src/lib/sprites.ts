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
	const urls = [
		// Only some species have female sprites; the others fall through.
		...(mon.female ? names.map((n) => `${dir}female/${n}.png`) : []),
		...names.map((n) => `${dir}${n}.png`)
	];
	return [...new Set(urls)];
}
