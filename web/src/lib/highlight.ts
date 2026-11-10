import hljs from 'highlight.js/lib/common';
import nix from 'highlight.js/lib/languages/nix';

hljs.registerLanguage('nix', nix);

const aliases: Record<string, string> = {
	svelte: 'xml',
	vue: 'xml',
	nu: 'bash',
	fish: 'bash',
	zsh: 'bash',
	jsonc: 'json',
	jsonl: 'json',
	lock: 'json',
	mjs: 'javascript',
	cjs: 'javascript',
	mts: 'typescript',
	cts: 'typescript'
};

export function escape(text: string): string {
	return text
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}

export function languageOf(path: string): string {
	const name = path.split('/').at(-1) ?? '';
	return name.includes('.') ? (name.split('.').at(-1) ?? '') : name;
}

export function highlight(code: string, language: string): string {
	const name = aliases[language.toLowerCase()] ?? language.toLowerCase();
	if (!hljs.getLanguage(name)) return escape(code);
	return hljs.highlight(code, { language: name, ignoreIllegals: true }).value;
}

export function highlightLines(code: string, language: string): string[] {
	const open: string[] = [];
	return highlight(code, language)
		.split('\n')
		.map((line) => {
			const prefix = open.join('');
			for (const [tag] of line.matchAll(/<span[^>]*>|<\/span>/g)) {
				if (tag === '</span>') open.pop();
				else open.push(tag);
			}
			return prefix + line + '</span>'.repeat(open.length);
		});
}
