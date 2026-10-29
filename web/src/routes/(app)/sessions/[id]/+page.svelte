<script lang="ts">
	import { page } from '$app/state';
	import { cancelTurn, destroyVm, getSettings, startTurn, startVm } from '$lib/api';
	import Composer from '$lib/components/Composer.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import VmPane from '$lib/components/VmPane.svelte';
	import { MonitorIcon, PlayIcon } from '$lib/icons';
	import { Session } from '$lib/session.svelte';
	import { onMount } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	const session = $derived(new Session(page.params.id ?? ''));
	let hidden = $state(false);
	let sendFailure = $state<string | null>(null);
	let modelMissing = $state(false);

	onMount(async () => {
		modelMissing = !(await getSettings()).model;
	});

	const active = $derived(session.vm.state !== 'stopped');
	const startable = $derived(
		session.vm.built && (session.vm.state === 'stopped' || session.vm.state === 'failed')
	);

	$effect(() => {
		const current = session;
		return () => current.close();
	});

	$effect(() => {
		if (session.vm.state === 'building' || session.vm.state === 'booting') {
			hidden = false;
		}
	});

	async function send(text: string) {
		sendFailure = null;
		try {
			await startTurn(session.id, text);
		} catch (error) {
			sendFailure = error instanceof Error ? error.message : String(error);
		}
	}
</script>

<div class="session">
	<div class="chat">
		<header>
			<span class="title">{session.title || 'New session'}</span>
			{#if startable}
				<button class="icon-button" title="Start VM" onclick={() => startVm(session.id)}>
					<PlayIcon size={16} />
				</button>
			{/if}
			{#if active && hidden}
				<button class="icon-button" title="Show VM" onclick={() => (hidden = false)}>
					<MonitorIcon size={16} />
				</button>
			{/if}
		</header>
		<Timeline
			entries={session.entries}
			busy={session.busy}
			failure={sendFailure ?? session.failure}
		/>
		{#if modelMissing}
			<div class="notice">
				No model is configured. <a href="/settings">Choose one in settings</a>
			</div>
		{/if}
		<Composer busy={session.busy} onsend={send} onstop={() => cancelTurn(session.id)} />
	</div>

	{#if active && !hidden}
		<div class="pane" transition:fly={{ x: 80, duration: 200, easing: cubicOut }}>
			<VmPane
				session={session.id}
				status={session.vm}
				boots={session.boots}
				onhide={() => (hidden = true)}
				ondestroy={() => destroyVm(session.id)}
			/>
		</div>
	{/if}
</div>

<style>
	.session {
		flex: 1;
		min-width: 0;
		display: flex;
	}

	.chat {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	header {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 44px;
		padding: 0 12px 0 24px;
		flex-shrink: 0;
		border-bottom: 1px solid var(--light);
	}

	.title {
		flex: 1;
		font-size: var(--font-title);
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.notice {
		width: 100%;
		max-width: 820px;
		margin: 0 auto;
		padding: 0 28px 8px;
		color: var(--text-secondary);
	}

	.notice a {
		color: var(--accent);
	}

	.pane {
		width: min(50%, 960px);
		flex-shrink: 0;
	}
</style>
