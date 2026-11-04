import { SvelteSet } from 'svelte/reactivity';
import {
	createSession,
	deleteSession,
	listEvents,
	type ListEvent,
	type NewEnvironment,
	type NodeSummary,
	type Selection,
	type SessionSummary
} from './api';

const UNSEEN_KEY = 'tsoup.unseen';

class SessionList {
	items = $state<SessionSummary[]>([]);
	nodes = $state<NodeSummary[]>([]);
	loaded = $state(false);
	unseen = new SvelteSet<string>(load());
	#unconfirmed = new Set<string>();
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

	async create(
		environment: NewEnvironment,
		selection: Selection,
		message: string
	): Promise<SessionSummary> {
		const session = await createSession(environment, selection, message);
		this.#unconfirmed.add(session.id);
		this.#add(session);
		return session;
	}

	async remove(id: string) {
		await deleteSession(id);
		this.#forget(id);
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
				const unconfirmed = this.items.filter(
					(session) => this.#unconfirmed.has(session.id) && !ids.has(session.id)
				);
				for (const id of ids) this.#unconfirmed.delete(id);
				this.items = event.sessions;
				for (const session of unconfirmed) this.#add(session);
				this.nodes = event.nodes;
				this.loaded = true;
				save(this.unseen);
				break;
			}
			case 'created':
				this.#unconfirmed.delete(event.session.id);
				this.#add(event.session);
				break;
			case 'removed':
				this.#unconfirmed.delete(event.id);
				this.#forget(event.id);
				break;
			case 'busy': {
				const session = this.items.find((session) => session.id === event.id);
				if (!session) break;
				if (session.busy && !event.busy) this.#finished(session.id);
				session.busy = event.busy;
				save(this.unseen);
				break;
			}
			case 'environment': {
				const session = this.items.find((session) => session.id === event.id);
				if (session) session.environment = event.environment;
				break;
			}
			case 'nodes':
				this.nodes = event.nodes;
				break;
		}
	}

	#add(session: SessionSummary) {
		if (this.items.some((item) => item.id === session.id)) return;
		const index = this.items.findIndex((item) => item.id < session.id);
		this.items.splice(index === -1 ? this.items.length : index, 0, session);
	}

	#forget(id: string) {
		this.items = this.items.filter((session) => session.id !== id);
		if (this.unseen.delete(id)) save(this.unseen);
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
