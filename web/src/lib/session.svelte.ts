import {
	listEntries,
	sessionEvents,
	type Action,
	type AgentSummary,
	type Context,
	type Entry,
	type EnvironmentStatus,
	type Progress,
	type Question,
	type Selection,
	type SessionEvent,
	type UserEntry
} from './api';
import { sessions } from './sessions.svelte';

export type Draft = {
	thinking: string;
	text: string;
	tool: string | null;
};

export class Thread {
	entries = $state<Entry[]>([]);
	start = $state(0);
	draft = $state<Draft | null>(null);
	compacting = $state(false);
	failure = $state<string | null>(null);

	context = $derived.by((): Context | null => {
		for (const entry of this.entries.toReversed()) {
			if (entry.kind === 'compaction') return null;
			if (entry.kind === 'assistant' && entry.context) return entry.context;
		}
		return null;
	});

	readonly session: string;
	readonly agent: string | null;
	#loaded: boolean;
	#queued: { index: number; entry: Entry }[] = [];

	constructor(session: string, agent: string | null) {
		this.session = session;
		this.agent = agent;
		this.#loaded = agent === null;
	}

	async load() {
		this.#loaded = false;
		try {
			const page = await listEntries(this.session, Number.MAX_SAFE_INTEGER, this.agent);
			this.merge(page.start, page.entries);
		} catch (error) {
			this.failure = error instanceof Error ? error.message : String(error);
		}
		this.#loaded = true;
		for (const { index, entry } of this.#queued.splice(0)) this.push(index, entry);
	}

	async loadOlder() {
		if (this.start === 0) return;
		try {
			const page = await listEntries(this.session, this.start, this.agent);
			if (page.start + page.entries.length !== this.start) return;
			this.entries = [...page.entries, ...this.entries];
			this.start = page.start;
		} catch (error) {
			this.failure = error instanceof Error ? error.message : String(error);
		}
	}

	merge(start: number, entries: Entry[]) {
		const gap = start - this.start;
		const kept = gap > 0 && gap <= this.entries.length ? this.entries.slice(0, gap) : [];
		this.entries = [...kept, ...entries];
		this.start = start - kept.length;
	}

	push(index: number, entry: Entry) {
		if (!this.#loaded) {
			this.#queued.push({ index, entry });
			return;
		}
		const end = this.start + this.entries.length;
		if (index < end) return;
		if (entry.kind === 'assistant') this.draft = null;
		if (entry.kind === 'compaction') this.compacting = false;
		if (index > end) {
			this.entries = [entry];
			this.start = index;
		} else {
			this.entries.push(entry);
		}
	}

	progress(progress: Progress) {
		this.draft ??= { thinking: '', text: '', tool: null };
		if (progress.kind === 'thinking') this.draft.thinking += progress.text;
		else if (progress.kind === 'text') this.draft.text += progress.text;
		else this.draft.tool = progress.name;
	}

	rewind(length: number) {
		this.entries = this.entries.slice(0, Math.max(0, length - this.start));
		this.start = Math.min(this.start, length);
	}

	clear() {
		this.entries = [];
		this.start = 0;
		this.settle();
	}

	settle() {
		this.draft = null;
		this.compacting = false;
	}
}

export class Session {
	title = $state('');
	actions = $state<Action[]>([]);
	selection = $state<Selection>({ model: null, effort: null });
	environment = $state<EnvironmentStatus | null>(null);
	busy = $state(false);
	failure = $state<string | null>(null);
	boots = $state(0);
	agents = $state<AgentSummary[]>([]);
	questions = $state<Question[]>([]);
	queued = $state<Record<string, UserEntry[]>>({});

	vm = $derived(this.environment?.kind === 'sandbox' ? this.environment.vm : null);

	readonly id: string;
	readonly main: Thread;
	#threads = new Map<string, Thread>();
	#events: EventSource;

	constructor(id: string) {
		this.id = id;
		this.main = new Thread(id, null);
		this.#events = sessionEvents(id);
		this.#events.onmessage = (message) => this.#apply(JSON.parse(message.data));
	}

	close() {
		this.#events.close();
	}

	thread(agent: string): Thread {
		let thread = this.#threads.get(agent);
		if (!thread) {
			thread = new Thread(this.id, agent);
			this.#threads.set(agent, thread);
			thread.load();
		}
		return thread;
	}

	#find(agent: string): Thread | undefined {
		return agent === this.id ? this.main : this.#threads.get(agent);
	}

	#settle() {
		this.main.settle();
		for (const thread of this.#threads.values()) thread.settle();
	}

	#apply(event: SessionEvent) {
		switch (event.type) {
			case 'snapshot':
				this.title = event.title;
				this.main.merge(event.start, event.entries);
				this.actions = event.actions;
				this.selection = event.selection;
				this.busy = event.busy;
				this.agents = event.agents;
				this.questions = event.questions;
				this.queued = event.queued;
				this.#settle();
				for (const thread of this.#threads.values()) thread.load();
				this.#setEnvironment(event.environment);
				break;
			case 'entry':
				this.#find(event.agent)?.push(event.index, event.entry);
				if (!this.title && event.agent === this.id && event.entry.kind === 'user') {
					this.title = titleOf(event.entry.text);
					sessions.rename(this.id, this.title);
				}
				break;
			case 'turn_started':
				this.busy = true;
				this.failure = null;
				break;
			case 'turn_finished':
			case 'turn_cancelled':
				this.busy = false;
				this.#settle();
				break;
			case 'turn_failed':
				this.busy = false;
				this.#settle();
				this.failure = event.message;
				break;
			case 'progress':
				this.#find(event.agent)?.progress(event.progress);
				break;
			case 'title':
				this.title = event.title;
				break;
			case 'cleared':
				this.main.clear();
				this.#threads.clear();
				this.agents = [];
				this.questions = [];
				this.queued = {};
				break;
			case 'rewind':
				this.main.rewind(event.length);
				break;
			case 'compacting': {
				const thread = this.#find(event.agent);
				if (thread) thread.compacting = true;
				break;
			}
			case 'agent': {
				const index = this.agents.findIndex((agent) => agent.id === event.agent.id);
				if (index === -1) this.agents.push(event.agent);
				else this.agents[index] = event.agent;
				if (event.agent.state !== 'running') this.#threads.get(event.agent.id)?.settle();
				break;
			}
			case 'questions':
				this.questions = event.questions;
				break;
			case 'queued':
				this.queued[event.agent] = event.entries;
				break;
			case 'actions':
				this.actions = event.actions;
				break;
			case 'selection':
				this.selection = event.selection;
				break;
			case 'environment':
				this.#setEnvironment(event.environment);
				break;
		}
	}

	#setEnvironment(environment: EnvironmentStatus) {
		if (
			environment.kind === 'sandbox' &&
			environment.vm.state === 'running' &&
			this.vm?.state !== 'running'
		) {
			this.boots += 1;
		}
		this.environment = environment;
	}
}

function titleOf(text: string): string {
	const line = text.split('\n').find((line) => line.trim()) ?? '';
	return Array.from(line.trim()).slice(0, 60).join('');
}
