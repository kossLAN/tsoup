import adapter from '@sveltejs/adapter-static';

const out = process.env.TSOUP_WEB_OUT ?? 'build';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		runes: true
	},
	kit: {
		adapter: adapter({ pages: out, assets: out, fallback: 'index.html' })
	}
};

export default config;
