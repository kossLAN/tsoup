import { createSession, deleteSession, listSessions, type SessionSummary } from './api';

class SessionList {
	items = $state<SessionSummary[]>([]);

	async refresh() {
		this.items = await listSessions();
	}

	async create(): Promise<SessionSummary> {
		const session = await createSession();
		this.items.unshift(session);
		return session;
	}

	async remove(id: string) {
		await deleteSession(id);
		this.items = this.items.filter((session) => session.id !== id);
	}

	rename(id: string, title: string) {
		const session = this.items.find((session) => session.id === id);
		if (session) {
			session.title = title;
		}
	}
}

export const sessions = new SessionList();
