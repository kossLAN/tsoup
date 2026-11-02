<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		cancelTurn,
		destroyVm,
		patchSession,
		runAction,
		startTurn,
		startVm,
		type Selection
	} from '$lib/api';
	import Composer from '$lib/components/Composer.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import ModelPicker from '$lib/components/ModelPicker.svelte';
	import PreviewPane from '$lib/components/PreviewPane.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import { DesktopTowerIcon, FolderIcon, MonitorIcon, PlayIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { Session } from '$lib/session.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	const session = $derived(new Session(page.params.id ?? ''));
	let hidden = $state(false);
	let failure = $state<string | null>(null);

	const vm = $derived(session.vm);
	const active = $derived(vm !== null && vm.state !== 'stopped');
	const startable = $derived(
		vm !== null && vm.built && (vm.state === 'stopped' || vm.state === 'failed')
	);
	const node = $derived(session.environment?.kind === 'node' ? session.environment : null);
	const directory = $derived(sessions.nodes.find(({ name }) => name === node?.name)?.directory);

	$effect(() => {
		const current = session;
		return () => current.close();
	});

	$effect(() => {
		sessions.view(session.id);
		return () => sessions.view(null);
	});

	$effect(() => {
		if (sessions.loaded && !sessions.items.some((item) => item.id === session.id)) {
			goto('/');
		}
	});

	$effect(() => {
		const current = session;
		const text = sessions.pending.get(current.id);
		if (text === undefined) return;
		sessions.pending.delete(current.id);
		current.ready.then(() => send(text, current.id));
	});

	$effect(() => {
		if (vm?.state === 'building' || vm?.state === 'booting') {
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

	async function select(selection: Selection) {
		failure = null;
		try {
			await patchSession(session.id, { selection });
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
			{#if node}
				<span
					class="flex items-center gap-1.5 px-1.5 text-label text-secondary"
					{@attach tooltip(directory ? `${directory}/${node.folder}` : node.folder)}
				>
					<DesktopTowerIcon size={14} />
					{node.name}
					<FolderIcon size={14} />
					{node.folder}
				</span>
			{/if}
			{#if startable}
				<button
					class="icon-button"
					onclick={() => startVm(session.id)}
					{@attach tooltip('Start VM')}
				>
					<PlayIcon size={16} />
				</button>
			{/if}
			{#if active && hidden}
				<button
					class="icon-button"
					onclick={() => (hidden = false)}
					{@attach tooltip('Show VM')}
				>
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
		<Composer busy={session.busy} onsend={send} onstop={() => cancelTurn(session.id)}>
			{#snippet accessory()}
				<ModelPicker selection={session.selection} onchange={select} />
			{/snippet}
		</Composer>
	</div>

	{#if vm && active && !hidden}
		<div
			class="w-1/2 max-w-240 shrink-0"
			transition:fly={{ x: 80, duration: 200, easing: cubicOut }}
		>
			<PreviewPane
				session={session.id}
				status={vm}
				boots={session.boots}
				actions={session.actions}
				onaction={run}
				onhide={() => (hidden = true)}
				ondestroy={() => destroyVm(session.id)}
			/>
		</div>
	{/if}
</div>
