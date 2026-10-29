import { sessionEvents, type Entry, type SessionEvent, type VmStatus } from './api';
import { sessions } from './sessions.svelte';

export class Session {
	title = $state('');
	entries = $state<Entry[]>([]);
	vm = $state<VmStatus>({ state: 'stopped' });
	busy = $state(false);
	failure = $state<string | null>(null);
	boots = $state(0);

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

	#apply(event: SessionEvent) {
		switch (event.type) {
			case 'snapshot':
				this.title = event.title;
				this.entries = event.entries;
				this.busy = event.busy;
				this.#setVm(event.vm);
				break;
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
