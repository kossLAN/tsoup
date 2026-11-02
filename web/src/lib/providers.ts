import type { ProviderKind } from '$lib/api';

export type Brand = {
	label: string;
	kind: ProviderKind;
	url: string;
	icon: string;
};

export const kinds: Record<ProviderKind, { label: string; url: string }> = {
	chat_completions: { label: 'OpenAI compatible', url: 'https://api.openai.com/v1' },
	responses: { label: 'OpenAI Responses', url: 'https://api.openai.com/v1' },
	anthropic: { label: 'Anthropic', url: 'https://api.anthropic.com' }
};

export const order: ProviderKind[] = ['chat_completions', 'responses', 'anthropic'];

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
		label: 'Kimi Code',
		kind: 'chat_completions',
		url: 'https://api.kimi.ai/coding/v1',
		icon: '/providers/kimi.svg'
	}
];

export function brand(provider: { kind: ProviderKind; url: string }): Brand | undefined {
	return catalog.find((entry) => entry.kind === provider.kind && entry.url === provider.url);
}
