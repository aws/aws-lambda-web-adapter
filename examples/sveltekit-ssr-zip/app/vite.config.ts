import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

// SvelteKit 3 no longer reads svelte.config.js; configuration is passed to the plugin.
// See https://svelte.dev/docs/kit/adapter-node for adapter options.
export default defineConfig({
	plugins: [sveltekit({ preprocess: vitePreprocess(), adapter: adapter() })]
});
