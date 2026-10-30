import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd());

	return {
		plugins: [tailwindcss(), sveltekit()],
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
					target: `http://127.0.0.1:${env.VITE_BACKEND_PORT}`,
					ws: true
				}
			}
		}
	};
});
