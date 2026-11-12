import { describe, expect, it } from 'vitest';
import { SPRITES_BASE, formSuffix, itemIconUrl, spriteUrls } from './sprites.js';

const mon = { species: 25, form: 0, female: false, shiny: false, egg: false };
const pt = `${SPRITES_BASE}/generation-iv/platinum/`;

describe('spriteUrls', () => {
	it('points at the game folder, pinned to a commit', () => {
		expect(spriteUrls('pt', mon)).toEqual([`${pt}25.png`]);
		expect(SPRITES_BASE).toMatch(/@[0-9a-f]{40}\//);
		expect(spriteUrls('b2w2', mon)[0]).toContain('/generation-v/black-white/25.png');
	});

	it('tries the most specific sprite first', () => {
		expect(spriteUrls('pt', { ...mon, female: true, shiny: true })).toEqual([
			`${pt}shiny/female/25.png`,
			`${pt}shiny/25.png`
		]);
		expect(spriteUrls('pt', { ...mon, species: 422, form: 1, shiny: true })).toEqual([
			`${pt}shiny/422-east.png`,
			`${pt}shiny/422.png`
		]);
	});

	it('asks for a female sprite only where the repository has one', () => {
		// Bulbasaur looks the same either way (a request for female/1.png would 404).
		expect(spriteUrls('pt', { ...mon, species: 1, female: true })).toEqual([`${pt}1.png`]);
		// Unfezant's female sprite exists in Black and White only (Gen 5 species).
		expect(spriteUrls('bw', { ...mon, species: 521, female: true })[0]).toContain(
			'/black-white/female/521.png'
		);
		expect(spriteUrls('pt', { ...mon, species: 400, female: true })[0]).toBe(`${pt}female/400.png`);
	});

	it('has none for eggs, empty slots and species the game lacks', () => {
		expect(spriteUrls('pt', { ...mon, egg: true })).toEqual([]);
		expect(spriteUrls('pt', { ...mon, species: 0 })).toEqual([]);
		expect(spriteUrls('hgss', { ...mon, species: 494 })).toEqual([]);
		expect(spriteUrls('bw', { ...mon, species: 649 })).toHaveLength(1);
		expect(spriteUrls('rs', mon)).toEqual([]);
	});
});

describe('formSuffix', () => {
	it('names Unown, Arceus and the other forms', () => {
		expect(formSuffix(201, 0, 4)).toBe('');
		expect(formSuffix(201, 1, 4)).toBe('b');
		expect(formSuffix(201, 27, 4)).toBe('question');
		// Gen 4 has the ??? type at 9; Gen 5 doesn't.
		expect(formSuffix(493, 9, 4)).toBe('unknown');
		expect(formSuffix(493, 9, 5)).toBe('fire');
		expect(formSuffix(493, 17, 4)).toBe('dark');
		expect(formSuffix(479, 2, 4)).toBe('wash');
		expect(formSuffix(585, 3, 5)).toBe('winter');
		expect(formSuffix(25, 3, 4)).toBe('');
	});
});

describe('itemIconUrl', () => {
	const file = (n: string) => itemIconUrl(n)?.split('/').pop();
	it('turns English item names into file names', () => {
		expect(file('Max Repel')).toBe('max-repel.png');
		expect(file("King's Rock")).toBe('kings-rock.png');
		expect(file('Poké Ball')).toBe('poke-ball.png');
		expect(file('NeverMeltIce')).toBe('never-melt-ice.png');
		expect(file('BlackGlasses')).toBe('black-glasses.png');
		expect(file('HP Up')).toBe('hp-up.png');
		expect(file('X Sp. Def')).toBe('x-sp-def.png');
	});
	it('gives TMs and HMs a disc, and nothing for no name', () => {
		expect(file('TM01')).toBe('tm-normal.png');
		expect(file('HM 05')).toBe('hm-normal.png');
		expect(itemIconUrl(undefined)).toBeUndefined();
		expect(itemIconUrl('???')).toBeUndefined();
	});
});
