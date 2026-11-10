<script lang="ts">
	import { getSettings, patchSettings, type Settings, type VmSettings } from '$lib/api';
	import Switch from '$lib/components/Switch.svelte';
	import { WarningCircleIcon } from '$lib/icons';
	import { onMount } from 'svelte';

	const megabytes = 1024;

	let settings = $state<Settings | null>(null);
	let failure = $state<string | null>(null);

	onMount(async () => {
		settings = await getSettings();
	});

	async function patch(change: Partial<VmSettings>) {
		if (!settings) return;
		failure = null;
		try {
			settings = await patchSettings({ vm: { ...settings.vm, ...change } });
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}

	function positive(value: string): number | null {
		const number = Number(value);
		return Number.isFinite(number) && number > 0 ? number : null;
	}

	function setMemory(value: string) {
		const gigabytes = positive(value);
		if (gigabytes !== null) patch({ memory: Math.round(gigabytes * megabytes) });
	}

	function setCpus(value: string) {
		const cpus = positive(value);
		if (cpus !== null) patch({ cpus: Math.round(cpus) });
	}
</script>

{#snippet row(title: string, description: string)}
	<span class="min-w-0 flex-1">
		<span class="block text-title font-medium">{title}</span>
		<span class="block text-label text-secondary">{description}</span>
	</span>
{/snippet}

{#if settings}
	{@const vm = settings.vm}
	<div class="mx-auto w-full max-w-170 px-6 py-6">
		<div class="pb-3 pl-1">
			<h2 class="text-title font-medium">Virtual machines</h2>
			<p class="text-secondary">
				Hardware every session's VM gets, applied the next time a VM is built
			</p>
		</div>

		<div class="flex flex-col rounded-surface surface p-1">
			<label class="flex items-center gap-3 px-3 py-2.5">
				{@render row('Memory', 'Gigabytes of RAM for each VM')}
				<input
					class="input w-20 flex-none text-right"
					type="number"
					min="0.5"
					step="0.5"
					value={vm.memory / megabytes}
					onchange={(event) => setMemory(event.currentTarget.value)}
				/>
			</label>
			<label class="flex items-center gap-3 px-3 py-2.5">
				{@render row('Processors', 'Virtual CPU cores for each VM')}
				<input
					class="input w-20 flex-none text-right"
					type="number"
					min="1"
					step="1"
					value={vm.cpus}
					onchange={(event) => setCpus(event.currentTarget.value)}
				/>
			</label>
			<div class="flex items-center gap-3 px-3 py-2.5">
				{@render row(
					'GPU acceleration',
					'Render the VM desktop with the host GPU, needs a GPU this machine can use headless'
				)}
				<Switch
					checked={vm.gpu}
					label="GPU acceleration"
					onchange={(gpu) => patch({ gpu })}
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
