import { listEntries, sessionEvents, type Entry, type SessionEvent, type VmStatus } from './api';
import { sessions } from './sessions.svelte';

export class Session {
	title = $state('');
	entries = $state<Entry[]>([]);
	start = $state(0);
	vm = $state<VmStatus>({ state: 'stopped', built: false });
	busy = $state(false);
	failure = $state<string | null>(null);
	boots = $state(0);

	readonly id: string;
	#events: EventSource;
	#ready = Promise.withResolvers<void>();
	readonly ready = this.#ready.promise;

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
				this.busy = event.busy;
				this.#setVm(event.vm);
				this.#ready.resolve();
				break;
			}
			case 'entry':
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
				break;
			case 'turn_failed':
				this.busy = false;
				this.failure = event.message;
				break;
			case 'vm':
				this.#setVm(event.status);
				break;
		}
	}

	#setVm(status: VmStatus) {
		if (status.state === 'running' && this.vm.state !== 'running') {
			this.boots += 1;
		}
		this.vm = status;
	}
}

function titleOf(text: string): string {
	const line = text.split('\n').find((line) => line.trim()) ?? '';
	return Array.from(line.trim()).slice(0, 60).join('');
}
