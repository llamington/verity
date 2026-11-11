import prettier from 'eslint-config-prettier';
import path from 'node:path';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, includeIgnoreFile } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

const gitignorePath = path.resolve(import.meta.dirname, '.gitignore');

export default defineConfig(
	includeIgnoreFile(gitignorePath),
	js.configs.recommended,
	ts.configs.strictTypeChecked,
	ts.configs.stylisticTypeChecked,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: {
			globals: { ...globals.browser, ...globals.node },
			parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname }
		},
		rules: {
			// Interpolating counts, levels and IDs is fine; objects and unknowns still aren't.
			'@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
			// typescript-eslint strongly recommend that you do not use the no-undef lint rule on TypeScript projects.
			// see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
			'no-undef': 'off'
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser
			}
		}
	},
	{
		// Override or add rule settings here, such as:
		// 'svelte/button-has-type': 'error'
		rules: {}
	},
	{
		// shadcn-svelte components are generated and re-added by its CLI, so they get the
		// untyped rules only. They pass `href` through from callers, which resolve it.
		files: ['src/lib/components/ui/**'],
		extends: [ts.configs.disableTypeChecked],
		rules: { 'svelte/no-navigation-without-resolve': 'off' }
	}
);
