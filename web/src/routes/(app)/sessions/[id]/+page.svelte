<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		cancelTurn,
		clearSession,
		compactSession,
		destroyVm,
		getSettings,
		patchSession,
		regenerate,
		runAction,
		startTurn,
		startVm,
		type Image,
		type Selection
	} from '$lib/api';
	import Composer from '$lib/components/Composer.svelte';
	import ContextMeter from '$lib/components/ContextMeter.svelte';
	import Menu from '$lib/components/Menu.svelte';
	import MenuItem from '$lib/components/MenuItem.svelte';
	import ModelNotice from '$lib/components/ModelNotice.svelte';
	import ModelPicker from '$lib/components/ModelPicker.svelte';
	import PreviewPane from '$lib/components/PreviewPane.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import SidePanel from '$lib/components/SidePanel.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import {
		BroomIcon,
		DesktopTowerIcon,
		DotsThreeIcon,
		FolderIcon,
		MonitorIcon,
		PlayIcon,
		TreeStructureIcon
	} from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { Session } from '$lib/session.svelte';
	import { sessions } from '$lib/sessions.svelte';
	import { cubicOut } from 'svelte/easing';
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';

	const session = $derived(new Session(page.params.id ?? ''));
	let hidden = $state(false);
	let panel = $state(false);
	let widths = $state({ preview: stored('preview'), activity: stored('activity') ?? 288 });
	let failure = $state<string | null>(null);
	let hideThinking = $state(true);
	let asked = 0;

	const drawer = { axis: 'x', duration: 200, easing: cubicOut } as const;

	const activity = $derived(session.agents.length > 0 || session.questions.length > 0);

	const vm = $derived(session.vm);
	const active = $derived(vm !== null && vm.state !== 'stopped');
	const startable = $derived(
		vm !== null && vm.built && (vm.state === 'stopped' || vm.state === 'failed')
	);
	const node = $derived(session.environment?.kind === 'node' ? session.environment : null);
	const directory = $derived(sessions.nodes.find(({ name }) => name === node?.name)?.directory);

	onMount(async () => {
		hideThinking = (await getSettings()).hide_thinking;
	});

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
		if (vm?.state === 'building' || vm?.state === 'booting') {
			hidden = false;
		}
	});

	$effect(() => {
		const waiting = session.questions.length;
		if (waiting > asked) panel = true;
		asked = waiting;
	});

	function stored(pane: string): number | null {
		const width = Number(localStorage.getItem(`tsoup.width.${pane}`));
		return width > 0 ? width : null;
	}

	function resize(pane: 'preview' | 'activity', width: number) {
		widths[pane] = width;
		localStorage.setItem(`tsoup.width.${pane}`, String(width));
	}

	async function send(text: string, images: Image[]): Promise<boolean> {
		failure = null;
		try {
			await startTurn(session.id, text, images);
			return true;
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
			return false;
		}
	}

	async function clear() {
		failure = null;
		try {
			await clearSession(session.id);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}

	async function compact() {
		failure = null;
		try {
			await compactSession(session.id);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}

	async function redo() {
		failure = null;
		try {
			await regenerate(session.id);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
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

<div class="@container flex min-w-0 flex-1">
	<div class="flex min-w-0 flex-1 flex-col">
		<header class="toolbar pr-3 pl-11 group-data-[sidebar=true]/shell:pl-4">
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
			<Menu placement="bottom-end">
				{#snippet trigger(attributes)}
					<button class="icon-button" {...attributes} {@attach tooltip('More')}>
						<DotsThreeIcon size={16} weight="bold" />
					</button>
				{/snippet}
				<MenuItem disabled={session.busy} onselect={clear}>
					<BroomIcon size={14} />
					Clear chat
				</MenuItem>
			</Menu>
			{#if activity && !panel}
				<button
					class="relative icon-button"
					onclick={() => (panel = true)}
					{@attach tooltip('Show activity')}
				>
					<TreeStructureIcon size={16} />
					{#if session.questions.length}
						<span class="absolute top-0.5 right-0.5 size-1.5 rounded-full bg-accent"
						></span>
					{/if}
				</button>
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
				entries={session.main.entries}
				start={session.main.start}
				busy={session.busy}
				draft={session.main.draft}
				queued={session.queued[session.id] ?? []}
				compacting={session.main.compacting}
				thinking={!hideThinking}
				failure={failure ?? session.failure ?? session.main.failure}
				onolder={() => session.main.loadOlder()}
				onregenerate={redo}
			/>
		{/key}
		<ModelNotice />
		<Composer busy={session.busy} onsend={send} onstop={() => cancelTurn(session.id)}>
			{#snippet accessory()}
				<ContextMeter
					context={session.main.context}
					disabled={session.busy || session.main.entries.length === 0}
					oncompact={compact}
				/>
				<ModelPicker selection={session.selection} onchange={select} />
			{/snippet}
		</Composer>
	</div>

	{#if vm && active && !hidden}
		<div class="relative shrink-0" transition:slide={drawer}>
			<Resizer min={320} onresize={(width) => resize('preview', width)} />
			<div
				class={['h-full', widths.preview === null && 'w-[50cqw] max-w-240']}
				style:width={widths.preview === null ? undefined : `${widths.preview}px`}
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
		</div>
	{/if}

	{#if activity && panel}
		<div class="relative shrink-0" transition:slide={drawer}>
			<Resizer min={220} onresize={(width) => resize('activity', width)} />
			<div class="h-full" style:width="{widths.activity}px">
				{#key session}
					<SidePanel {session} thinking={!hideThinking} onhide={() => (panel = false)} />
				{/key}
			</div>
		</div>
	{/if}
</div>
