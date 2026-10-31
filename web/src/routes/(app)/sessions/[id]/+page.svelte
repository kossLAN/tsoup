<script lang="ts">
	import { page } from '$app/state';
	import { cancelTurn, destroyVm, runAction, startTurn, startVm } from '$lib/api';
	import Composer from '$lib/components/Composer.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import PreviewPane from '$lib/components/PreviewPane.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import { MonitorIcon, PlayIcon } from '$lib/icons';
	import { Session } from '$lib/session.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	const session = $derived(new Session(page.params.id ?? ''));
	let hidden = $state(false);
	let failure = $state<string | null>(null);

	const active = $derived(session.vm.state !== 'stopped');
	const startable = $derived(
		session.vm.built && (session.vm.state === 'stopped' || session.vm.state === 'failed')
	);

	$effect(() => {
		const current = session;
		return () => current.close();
	});

	$effect(() => {
		const current = session;
		const text = sessions.pending.get(current.id);
		if (text === undefined) return;
		sessions.pending.delete(current.id);
		current.ready.then(() => send(text, current.id));
	});

	$effect(() => {
		if (session.vm.state === 'building' || session.vm.state === 'booting') {
			hidden = false;
		}
	});

	async function send(text: string, id = session.id): Promise<boolean> {
		failure = null;
		try {
			await startTurn(id, text);
			return true;
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
			return false;
		}
	}

	async function run(label: string) {
		failure = null;
		try {
			await runAction(session.id, label);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}
</script>

<div class="flex min-w-0 flex-1">
	<div class="flex min-w-0 flex-1 flex-col">
		<header class="toolbar pr-3 pl-11">
			<span class="flex-1 truncate text-title font-medium"
				>{session.title || 'New session'}</span
			>
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
		{#key session}
			<Timeline
				entries={session.entries}
				start={session.start}
				busy={session.busy}
				failure={failure ?? session.failure}
				onolder={() => session.loadOlder()}
			/>
		{/key}
		<ModelNotice />
		<Composer busy={session.busy} onsend={send} onstop={() => cancelTurn(session.id)} />
	</div>

	{#if active && !hidden}
		<div
			class="w-1/2 max-w-240 shrink-0"
			transition:fly={{ x: 80, duration: 200, easing: cubicOut }}
		>
			<PreviewPane
				session={session.id}
				status={session.vm}
				boots={session.boots}
				actions={session.actions}
				onaction={run}
				onhide={() => (hidden = true)}
				ondestroy={() => destroyVm(session.id)}
			/>
		</div>
	{/if}
</div>
