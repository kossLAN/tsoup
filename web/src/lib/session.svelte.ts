import {
	listEntries,
	sessionEvents,
	type Action,
	type Entry,
	type EnvironmentStatus,
	type Progress,
	type Selection,
	type SessionEvent
} from './api';
import { sessions } from './sessions.svelte';

export type Draft = {
	thinking: string;
	text: string;
	tool: string | null;
};

export class Session {
	title = $state('');
	entries = $state<Entry[]>([]);
	actions = $state<Action[]>([]);
	selection = $state<Selection>({ model: null, effort: null });
	draft = $state<Draft | null>(null);
	start = $state(0);
	environment = $state<EnvironmentStatus | null>(null);
	busy = $state(false);
	failure = $state<string | null>(null);
	boots = $state(0);

	vm = $derived(this.environment?.kind === 'sandbox' ? this.environment.vm : null);

	readonly id: string;
	#events: EventSource;

	constructor(id: string) {
		this.id = id;
		this.#events = sessionEvents(id);
		this.#events.onmessage = (message) => this.#apply(JSON.parse(message.data));
	}

	close() {
		this.#events.close();
	}

	async loadOlder() {
		if (this.start === 0) return;
		try {
			const page = await listEntries(this.id, this.start);
			if (page.start + page.entries.length !== this.start) return;
			this.entries = [...page.entries, ...this.entries];
			this.start = page.start;
		} catch (error) {
			this.failure = error instanceof Error ? error.message : String(error);
		}
	}

	#apply(event: SessionEvent) {
		switch (event.type) {
			case 'snapshot': {
				const gap = event.start - this.start;
				const kept =
					gap > 0 && gap <= this.entries.length ? this.entries.slice(0, gap) : [];
				this.title = event.title;
				this.entries = [...kept, ...event.entries];
				this.start = event.start - kept.length;
				this.actions = event.actions;
				this.selection = event.selection;
				this.busy = event.busy;
				this.draft = null;
				this.#setEnvironment(event.environment);
				break;
			}
			case 'entry':
				if (event.entry.kind === 'assistant') this.draft = null;
				this.entries.push(event.entry);
				if (!this.title && event.entry.kind === 'user') {
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
				this.draft = null;
				break;
			case 'turn_failed':
				this.busy = false;
				this.draft = null;
				this.failure = event.message;
				break;
			case 'progress':
				this.#progress(event.progress);
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

	#progress(progress: Progress) {
		this.draft ??= { thinking: '', text: '', tool: null };
		if (progress.kind === 'thinking') this.draft.thinking += progress.text;
		else if (progress.kind === 'text') this.draft.text += progress.text;
		else this.draft.tool = progress.name;
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
