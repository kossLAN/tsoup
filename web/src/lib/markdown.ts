import MarkdownIt from 'markdown-it';
import { escape, highlight } from './highlight';

const markdown = new MarkdownIt({ html: false, linkify: true, breaks: true });

markdown.renderer.rules.link_open = (tokens, index, options, _env, self) => {
	tokens[index].attrSet('target', '_blank');
	tokens[index].attrSet('rel', 'noreferrer');
	return self.renderToken(tokens, index, options);
};

markdown.renderer.rules.fence = (tokens, index) => {
	const token = tokens[index];
	const language = token.info.trim().split(/\s+/)[0] ?? '';
	const code = highlight(token.content.replace(/\n$/, ''), language);
	return (
		'<div class="code">' +
		`<div class="code-bar"><span>${escape(language)}</span>` +
		'<button type="button" data-copy>Copy</button></div>' +
		`<pre><code>${code}</code></pre></div>`
	);
};

export function render(text: string): string {
	return markdown.render(text);
}
