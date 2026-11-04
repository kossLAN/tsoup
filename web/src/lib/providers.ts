import type { ProviderKind } from '$lib/api';

export type ApiKind = Exclude<ProviderKind, 'claude_code'>;

export type Brand = {
	label: string;
	kind: ProviderKind;
	url: string | null;
	icon: string;
};

export const labels: Record<ProviderKind, string> = {
	chat_completions: 'OpenAI compatible',
	responses: 'OpenAI Responses',
	anthropic: 'Anthropic',
	claude_code: 'Claude Code CLI'
};

export const examples: Record<ApiKind, string> = {
	chat_completions: 'https://api.openai.com/v1',
	responses: 'https://api.openai.com/v1',
	anthropic: 'https://api.anthropic.com'
};

export const order: ApiKind[] = ['chat_completions', 'responses', 'anthropic'];

export const catalog: Brand[] = [
	{
		label: 'OpenAI',
		kind: 'responses',
		url: 'https://api.openai.com/v1',
		icon: '/providers/openai.svg'
	},
	{
		label: 'Anthropic',
		kind: 'anthropic',
		url: 'https://api.anthropic.com',
		icon: '/providers/anthropic.svg'
	},
	{
		label: 'Claude Code',
		kind: 'claude_code',
		url: null,
		icon: '/providers/claude.svg'
	},
	{
		label: 'Kimi Code',
		kind: 'chat_completions',
		url: 'https://api.kimi.ai/coding/v1',
		icon: '/providers/kimi.svg'
	}
];

export function brand(provider: { kind: ProviderKind; url: string | null }): Brand | undefined {
	return catalog.find((entry) => entry.kind === provider.kind && entry.url === provider.url);
}
