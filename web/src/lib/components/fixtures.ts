// Sample data for Storybook stories.
import type { Choices, Gift, Summary } from '#lib/verity.js';

export const n = (id: number, name?: string) => ({ id, name });
export const summary: Summary = {
	mon: {
		species: 157,
		speciesName: 'Typhlosion',
		nickname: 'Cayenne',
		nicknamed: true,
		level: 100,
		form: 0,
		female: false,
		egg: false,
		shiny: false,
		checksumOk: true
	},
	pid: 0x9626da57,
	exp: 1059860,
	nature: n(16, 'Jolly'),
	ability: n(66, 'Blaze'),
	hiddenAbility: false,
	gender: 0,
	heldItem: n(0, 'None'),
	ball: n(4, 'Poké Ball'),
	types: [n(9, 'Fire')],
	moves: [
		{ id: 332, name: 'Aerial Ace', pp: 32, ppUps: 3, moveType: n(2, 'Flying') },
		{ id: 89, name: 'Earthquake', pp: 16, ppUps: 3, moveType: n(4, 'Ground') },
		{ id: 29, name: 'Headbutt', pp: 15, ppUps: 0, moveType: n(0, 'Normal') },
		{ id: 53, name: 'Flamethrower', pp: 24, ppUps: 3, moveType: n(9, 'Fire') }
	],
	ivs: [11, 3, 8, 23, 26, 29],
	evs: [252, 252, 0, 6, 0, 0],
	stats: [340, 239, 169, 251, 224, 204],
	friendship: 255,
	ot: 'RoC',
	otFemale: false,
	tid: 41896,
	sid: 35319,
	language: 'English',
	origin: 'HeartGold',
	metLocation: n(126, 'New Bark Town'),
	metLevel: 5,
	metDate: '2016-05-08',
	eggLocation: n(0),
	eggDate: undefined,
	fateful: false,
	contest: [0, 0, 0, 0, 0, 0],
	markings: [true, false, true, false, false, false],
	ribbonCount: 2,
	ribbons: Array.from({ length: 80 }, (_, i) => i === 0 || i === 17)
};

export const choices: Choices = {
	heldItems: [n(0), n(1, 'Master Ball'), n(234, 'Leftovers'), n(217, 'Quick Claw')],
	moves: [n(29, 'Headbutt'), n(53, 'Flamethrower'), n(89, 'Earthquake'), n(332, 'Aerial Ace')],
	balls: [n(1, 'Master Ball'), n(4, 'Poké Ball'), n(5, 'Safari Ball')],
	locations: [n(126, 'New Bark Town'), n(127, 'Cherrygrove City'), n(2000, 'Day-Care Couple')],
	ribbons: Array.from({ length: 80 }, (_, i) => `Ribbon ${i + 1}`),
	nicknameMax: 10,
	natures: ['Hardy', 'Lonely', 'Brave', 'Adamant', 'Naughty'],
	wildBalls: [n(4, 'Poké Ball'), n(3, 'Great Ball'), n(2, 'Ultra Ball')]
};

const gift = (record: number, over: Partial<Gift>): Gift => ({
	source: 0,
	record,
	title: 'Event',
	gives: 'Gift',
	kind: 'Gift',
	species: 0,
	item: 0,
	ot: undefined,
	tid: 0,
	sid: 0,
	region: undefined,
	cardId: undefined,
	ready: true,
	status: 'Ready',
	...over
});

export const gifts: Gift[] = [
	gift(1, {
		title: 'Shiny Pikachu',
		gives: 'Pikachu',
		kind: 'Pokémon',
		species: 25,
		ot: 'Nintendo',
		tid: 7,
		region: 'North America',
		cardId: 101
	}),
	gift(2, {
		title: 'Shiny Pikachu',
		gives: 'Pikachu',
		kind: 'Pokémon',
		species: 25,
		ot: 'Game',
		tid: 41212,
		region: 'Europe',
		cardId: 102
	}),
	gift(3, { title: 'Member Card', gives: 'Member Card', kind: 'Member Card', cardId: 7 }),
	gift(4, {
		title: 'Manaphy Egg',
		gives: 'Manaphy Egg',
		kind: 'Pokémon Egg',
		ready: false,
		status: 'Already received'
	})
];
