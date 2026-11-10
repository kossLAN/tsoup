<script lang="ts">
	import { steerAgent, type AgentSummary, type Image } from '$lib/api';
	import type { Session } from '$lib/session.svelte';
	import Composer from './Composer.svelte';
	import Timeline from './Timeline.svelte';

	let { session, agent, thinking }: { session: Session; agent: AgentSummary; thinking: boolean } =
		$props();

	const thread = $derived(session.thread(agent.id));
	const running = $derived(agent.state === 'running');

	let failure = $state<string | null>(null);

	async function steer(text: string, images: Image[]): Promise<boolean> {
		failure = null;
		try {
			await steerAgent(session.id, agent.id, text, images);
			return true;
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
			return false;
		}
	}
</script>

{#key thread}
	<Timeline
		entries={thread.entries}
		start={thread.start}
		busy={running}
		draft={thread.draft}
		compacting={thread.compacting}
		{thinking}
		failure={failure ?? thread.failure}
		onolder={() => thread.loadOlder()}
		dense
	/>
{/key}
{#if running}
	<Composer onsend={steer} placeholder="Steer the agent" dense />
{/if}
