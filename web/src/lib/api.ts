export type SessionSummary = {
	id: string;
	title: string;
	busy: boolean;
	environment: EnvironmentStatus;
};

export type NodeSummary = {
	name: string;
	directory: string;
};

export type EnvironmentStatus =
	{ kind: 'sandbox'; vm: VmStatus } | { kind: 'node'; name: string; folder: string };

export type NewEnvironment = { kind: 'sandbox' } | { kind: 'node'; name: string; folder: string };

export type ToolCall = {
	id: string;
	type: 'function';
	function: {
		name: string;
		arguments: string;
	};
};

export type FileDiff = {
	path: string;
	patch: string;
};

export type Entry =
	| { kind: 'user'; text: string; images?: Image[] }
	| {
			kind: 'assistant';
			thinking?: string;
			text: string | null;
			tool_calls: ToolCall[];
			context?: Context;
	  }
	| { kind: 'tool'; call_id: string; output: string; error: boolean; diff: FileDiff | null }
	| { kind: 'compaction'; summary: string };

export type Image = {
	media_type: string;
	data: string;
};

export type Context = {
	used: number;
	window: number;
};

export type AgentState = 'running' | 'finished' | 'failed' | 'stopped';

export type AgentSummary = {
	id: string;
	parent: string | null;
	name: string;
	state: AgentState;
	model?: ModelChoice;
};

export type Question = {
	id: string;
	agent: string;
	text: string;
	options: string[];
};

export type Preview = {
	name: string;
	label: string;
	path: string;
	target: number | null;
};

export type Action = {
	label: string;
	command: string;
};

export type VmStatus = (
	| { state: 'stopped' }
	| { state: 'building' }
	| { state: 'booting' }
	| { state: 'running'; previews: Preview[] }
	| { state: 'failed'; message: string }
) & { built: boolean };

export type SessionEvent =
	| {
			type: 'snapshot';
			title: string;
			start: number;
			entries: Entry[];
			actions: Action[];
			selection: Selection;
			environment: EnvironmentStatus;
			busy: boolean;
			agents: AgentSummary[];
			questions: Question[];
	  }
	| { type: 'entry'; agent: string; index: number; entry: Entry }
	| { type: 'turn_started' }
	| { type: 'turn_finished' }
	| { type: 'turn_failed'; message: string }
	| { type: 'turn_cancelled' }
	| { type: 'environment'; environment: EnvironmentStatus }
	| { type: 'actions'; actions: Action[] }
	| { type: 'selection'; selection: Selection }
	| { type: 'progress'; agent: string; progress: Progress }
	| { type: 'agent'; agent: AgentSummary }
	| { type: 'questions'; questions: Question[] }
	| { type: 'title'; title: string }
	| { type: 'cleared' }
	| { type: 'rewind'; length: number }
	| { type: 'compacting'; agent: string };

export type Progress =
	| { kind: 'thinking'; text: string }
	| { kind: 'text'; text: string }
	| { kind: 'tool_call'; name: string };

export type ListEvent =
	| { type: 'snapshot'; sessions: SessionSummary[]; nodes: NodeSummary[] }
	| { type: 'created'; session: SessionSummary }
	| { type: 'removed'; id: string }
	| { type: 'busy'; id: string; busy: boolean }
	| { type: 'title'; id: string; title: string }
	| { type: 'environment'; id: string; environment: EnvironmentStatus }
	| { type: 'nodes'; nodes: NodeSummary[] };

async function request(method: string, path: string, body?: unknown): Promise<Response> {
	const response = await fetch(path, {
		method,
		headers: body === undefined ? undefined : { 'content-type': 'application/json' },
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	if (!response.ok) {
		const failure: { error?: string } = await response.json().catch(() => ({}));
		throw new Error(failure.error ?? `${method} ${path} failed with ${response.status}`);
	}
	return response;
}

export function listEvents(): EventSource {
	return new EventSource('/api/events');
}

export async function listFolders(node: string): Promise<string[]> {
	return (await request('GET', `/api/nodes/${encodeURIComponent(node)}/folders`)).json();
}

export async function createSession(
	environment: NewEnvironment,
	selection: Selection,
	message: string,
	images: Image[] = []
): Promise<SessionSummary> {
	return (
		await request('POST', '/api/sessions', { environment, selection, message, images })
	).json();
}

export async function patchSession(
	id: string,
	patch: { selection?: Selection; title?: string }
): Promise<void> {
	await request('PATCH', `/api/sessions/${id}`, patch);
}

export async function clearSession(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}/entries`);
}

export async function deleteSession(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}`);
}

export type EntryPage = {
	start: number;
	entries: Entry[];
};

export async function listEntries(
	id: string,
	before: number,
	agent: string | null = null
): Promise<EntryPage> {
	const query = agent ? `before=${before}&agent=${agent}` : `before=${before}`;
	return (await request('GET', `/api/sessions/${id}/entries?${query}`)).json();
}

export async function startTurn(id: string, content: string, images: Image[] = []): Promise<void> {
	await request('POST', `/api/sessions/${id}/turns`, { content, images });
}

export async function cancelTurn(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}/turn`);
}

export async function compactSession(id: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/compact`);
}

export async function regenerate(id: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/regenerate`);
}

export async function steerAgent(
	id: string,
	agent: string,
	content: string,
	images: Image[] = []
): Promise<void> {
	await request('POST', `/api/sessions/${id}/agents/${agent}/messages`, { content, images });
}

export async function stopAgent(id: string, agent: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}/agents/${agent}`);
}

export async function answerQuestion(id: string, question: string, answer: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/questions/${question}`, { answer });
}

export async function startVm(id: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/vm`);
}

export async function destroyVm(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}/vm`);
}

export async function runAction(id: string, label: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/actions`, { label });
}

export function sessionEvents(id: string): EventSource {
	return new EventSource(`/api/sessions/${id}/events`);
}

export function previewUrl(id: string, preview: Preview): string {
	const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(location.hostname);
	const host = loopback ? 'localhost' : location.hostname;
	const port = import.meta.env.DEV ? import.meta.env.VITE_BACKEND_PORT : location.port;
	const origin = port ? `${host}:${port}` : host;
	return `${location.protocol}//${preview.name}.${id}.${origin}${preview.path}`;
}

export type ProviderKind = 'chat_completions' | 'responses' | 'anthropic' | 'claude_code';

export type Provider = {
	id: string;
	name: string;
	kind: ProviderKind;
	url: string | null;
	has_key: boolean;
};

export type ModelChoice = {
	provider: string;
	name: string;
};

export type Effort = 'low' | 'medium' | 'high' | 'xhigh';

export type Selection = {
	model: ModelChoice | null;
	effort: Effort | null;
};

export type Settings = {
	providers: Provider[];
	default_model: ModelChoice | null;
	last_effort: Effort | null;
	hide_thinking: boolean;
	agent_titles: boolean;
	prompts: { sandbox: Prompt; node: Prompt; subagent: Prompt };
	subagents: Subagents;
	compaction: Compaction;
};

export type Prompt = {
	text: string;
	default: string;
};

export type Compaction = {
	auto: boolean;
	window: number;
};

export type Subagents = {
	enabled: boolean;
	depth: number;
	model: ModelChoice | null;
	effort: Effort | null;
	models: ModelChoice[];
};

export type ProviderUpdate = {
	id: string | null;
	name: string;
	kind: ProviderKind;
	url: string | null;
	key?: string;
};

export type SettingsPatch = {
	providers?: ProviderUpdate[];
	default_model?: ModelChoice;
	hide_thinking?: boolean;
	agent_titles?: boolean;
	prompts?: { sandbox: string | null; node: string | null; subagent: string | null };
	subagents?: Subagents;
	compaction?: Compaction;
};

export async function getSettings(): Promise<Settings> {
	return (await request('GET', '/api/settings')).json();
}

export async function patchSettings(patch: SettingsPatch): Promise<Settings> {
	return (await request('PATCH', '/api/settings', patch)).json();
}

export async function listModels(provider: string): Promise<string[]> {
	return (await request('GET', `/api/providers/${provider}/models`)).json();
}

export type ClaudeCodeAccount = {
	signed_in: boolean;
	email: string | null;
	subscription: string | null;
};

export async function getClaudeCodeAccount(): Promise<ClaudeCodeAccount> {
	return (await request('GET', '/api/claude-code/account')).json();
}

export async function startClaudeCodeLogin(): Promise<{ url: string }> {
	return (await request('POST', '/api/claude-code/login')).json();
}

export async function finishClaudeCodeLogin(code: string): Promise<ClaudeCodeAccount> {
	return (await request('POST', '/api/claude-code/login/code', { code })).json();
}
