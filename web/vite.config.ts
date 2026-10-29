import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	build: {
		target: 'esnext'
	},
	optimizeDeps: {
		esbuildOptions: {
			target: 'esnext'
		}
	},
	server: {
		proxy: {
			'/api': {
				target: 'http://127.0.0.1:7878',
				ws: true
			}
		}
	}
});
