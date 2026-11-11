// Conventional Commits for the whole repo: checked by the commit-msg hook (.husky/) and by CI.
/** @type {import('@commitlint/types').UserConfig} */
export default {
	extends: ['@commitlint/config-conventional'],
	rules: {
		// The repo's parts. A scope is optional; when given it must be one of these.
		'scope-enum': [
			2,
			'always',
			[
				'core', // crates/verity
				'ffi', // crates/verity-ffi (the DS's C ABI)
				'wasm', // crates/verity-wasm
				'rom', // crates/verity-rom
				'datagen', // crates/datagen and data/
				'ds', // nds/ (the DS front end)
				'web', // web/
				'harness', // tools/emu-harness
				'ci',
				'deps',
				'handoff', // docs/research/handoff.md
				'plan' // PLAN.md
			]
		]
	}
};
