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
	| { kind: 'user'; text: string }
	| { kind: 'assistant'; text: string | null; tool_calls: ToolCall[] }
	| { kind: 'tool'; call_id: string; output: string; error: boolean; diff: FileDiff | null };

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
			environment: EnvironmentStatus;
			busy: boolean;
	  }
	| { type: 'entry'; entry: Entry }
	| { type: 'turn_started' }
	| { type: 'turn_finished' }
	| { type: 'turn_failed'; message: string }
	| { type: 'turn_cancelled' }
	| { type: 'environment'; environment: EnvironmentStatus }
	| { type: 'actions'; actions: Action[] };

export type ListEvent =
	| { type: 'snapshot'; sessions: SessionSummary[]; nodes: NodeSummary[] }
	| { type: 'created'; session: SessionSummary }
	| { type: 'removed'; id: string }
	| { type: 'busy'; id: string; busy: boolean }
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

export async function createSession(environment: NewEnvironment): Promise<SessionSummary> {
	return (await request('POST', '/api/sessions', environment)).json();
}

export async function deleteSession(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}`);
}

export type EntryPage = {
	start: number;
	entries: Entry[];
};

export async function listEntries(id: string, before: number): Promise<EntryPage> {
	return (await request('GET', `/api/sessions/${id}/entries?before=${before}`)).json();
}

export async function startTurn(id: string, content: string): Promise<void> {
	await request('POST', `/api/sessions/${id}/turns`, { content });
}

export async function cancelTurn(id: string): Promise<void> {
	await request('DELETE', `/api/sessions/${id}/turn`);
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

export type Settings = {
	llm_url: string;
	has_llm_key: boolean;
	model: string | null;
};

export type SettingsUpdate = {
	llm_url: string;
	llm_key?: string;
	model: string | null;
};

export async function getSettings(): Promise<Settings> {
	return (await request('GET', '/api/settings')).json();
}

export async function saveSettings(update: SettingsUpdate): Promise<Settings> {
	return (await request('PUT', '/api/settings', update)).json();
}

export async function listModels(): Promise<string[]> {
	return (await request('GET', '/api/models')).json();
}
