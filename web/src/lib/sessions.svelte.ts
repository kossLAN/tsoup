import { SvelteSet } from 'svelte/reactivity';
import {
	createSession,
	deleteSession,
	listEvents,
	type ListEvent,
	type SessionSummary
} from './api';

const UNSEEN_KEY = 'tsoup.unseen';

class SessionList {
	items = $state<SessionSummary[]>([]);
	unseen = new SvelteSet<string>(load());
	pending = new Map<string, string>();
	#viewing: string | null = null;

	connect(): () => void {
		const events = listEvents();
		events.onmessage = (message) => this.#apply(JSON.parse(message.data));
		const visible = () => this.#see();
		document.addEventListener('visibilitychange', visible);
		return () => {
			events.close();
			document.removeEventListener('visibilitychange', visible);
		};
	}

	view(id: string | null) {
		this.#viewing = id;
		this.#see();
	}

	async create(): Promise<SessionSummary> {
		const session = await createSession();
		this.items.unshift(session);
		return session;
	}

	async remove(id: string) {
		await deleteSession(id);
		this.items = this.items.filter((session) => session.id !== id);
		if (this.unseen.delete(id)) save(this.unseen);
	}

	rename(id: string, title: string) {
		const session = this.items.find((session) => session.id === id);
		if (session) {
			session.title = title;
		}
	}

	#apply(event: ListEvent) {
		switch (event.type) {
			case 'snapshot': {
				const before = new Map(this.items.map((session) => [session.id, session.busy]));
				for (const session of event.sessions) {
					if (before.get(session.id) && !session.busy) this.#finished(session.id);
				}
				const ids = new Set(event.sessions.map((session) => session.id));
				for (const id of this.unseen) {
					if (!ids.has(id)) this.unseen.delete(id);
				}
				this.items = event.sessions;
				save(this.unseen);
				break;
			}
			case 'busy': {
				const session = this.items.find((session) => session.id === event.id);
				if (!session) break;
				if (session.busy && !event.busy) this.#finished(session.id);
				session.busy = event.busy;
				save(this.unseen);
				break;
			}
			case 'vm': {
				const session = this.items.find((session) => session.id === event.id);
				if (session) session.vm = event.status;
				break;
			}
		}
	}

	#finished(id: string) {
		if (id !== this.#viewing || document.hidden) this.unseen.add(id);
	}

	#see() {
		if (this.#viewing && !document.hidden && this.unseen.delete(this.#viewing)) {
			save(this.unseen);
		}
	}
}

function load(): string[] {
	try {
		const stored: unknown = JSON.parse(localStorage.getItem(UNSEEN_KEY) ?? '[]');
		return Array.isArray(stored) ? stored.filter((id) => typeof id === 'string') : [];
	} catch {
		return [];
	}
}

function save(unseen: Set<string>) {
	try {
		localStorage.setItem(UNSEEN_KEY, JSON.stringify([...unseen]));
	} catch {}
}

export const sessions = new SessionList();
