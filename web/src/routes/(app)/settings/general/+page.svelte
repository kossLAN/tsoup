<script lang="ts">
	import { getSettings, patchSettings, type Settings, type SettingsPatch } from '$lib/api';
	import Switch from '$lib/components/Switch.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { onMount } from 'svelte';

	let settings = $state<Settings | null>(null);
	let failure = $state<string | null>(null);

	onMount(async () => {
		settings = await getSettings();
	});

	async function patch(change: SettingsPatch) {
		failure = null;
		try {
			settings = await patchSettings(change);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}
</script>

{#if settings}
	<div class="mx-auto w-full max-w-170 px-6 py-6">
		<div class="pb-3 pl-1">
			<h2 class="text-title font-medium">General</h2>
			<p class="text-secondary">How sessions look and behave</p>
		</div>

		<div class="flex flex-col rounded-surface surface p-1">
			<div class="flex items-center gap-3 px-3 py-2.5">
				<span class="min-w-0 flex-1">
					<span class="block text-title font-medium">Show thinking</span>
					<span class="block text-label text-secondary"
						>Stream the model's reasoning into the conversation</span
					>
				</span>
				<Switch
					checked={!settings.hide_thinking}
					label="Show thinking"
					onchange={(show) => patch({ hide_thinking: !show })}
				/>
			</div>
			<div class="flex items-center gap-3 px-3 py-2.5">
				<span class="min-w-0 flex-1">
					<span class="block text-title font-medium">Agent-written titles</span>
					<span class="block text-label text-secondary"
						>Have the model name a session from its first message</span
					>
				</span>
				<Switch
					checked={settings.agent_titles}
					label="Agent-written titles"
					onchange={(agent_titles) => patch({ agent_titles })}
				/>
			</div>
		</div>

		{#if failure}
			<div class="mt-3 alert whitespace-pre-wrap">
				<WarningCircleIcon size={14} />{failure}
			</div>
		{/if}
	</div>
{/if}
