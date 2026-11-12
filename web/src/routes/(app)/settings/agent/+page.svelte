<script lang="ts">
	import {
		getSettings,
		patchSettings,
		type ModelChoice,
		type Settings,
		type SettingsPatch,
		type Subagents
	} from '$lib/api';
	import ModelPicker from '$lib/components/ModelPicker.svelte';
	import Switch from '$lib/components/Switch.svelte';
	import { ArrowCounterClockwiseIcon, WarningCircleIcon, XIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import { onMount } from 'svelte';

	type Kind = 'sandbox' | 'node' | 'subagent';

	const prompts: { kind: Kind; title: string; description: string }[] = [
		{
			kind: 'sandbox',
			title: 'Sandbox',
			description: 'For sessions that work in a workspace with a VM'
		},
		{
			kind: 'node',
			title: 'Node',
			description:
				'For sessions on a node, {name} and {directory} become the machine and its folder'
		},
		{
			kind: 'subagent',
			title: 'Subagent',
			description: 'Added after the prompt above for every subagent'
		}
	];

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

	function setPrompt(kind: Kind, text: string | null) {
		if (!settings) return;
		const custom = (prompt: { text: string; default: string }) =>
			prompt.text === prompt.default ? null : prompt.text;
		patch({
			prompts: {
				sandbox: custom(settings.prompts.sandbox),
				node: custom(settings.prompts.node),
				subagent: custom(settings.prompts.subagent),
				[kind]: text
			}
		});
	}

	function setSubagents(change: Partial<Subagents>) {
		if (!settings) return;
		patch({ subagents: { ...settings.subagents, ...change } });
	}

	function setDepth(value: string) {
		const depth = Math.round(Number(value));
		if (Number.isFinite(depth) && depth > 0) setSubagents({ depth });
	}

	function setWindow(value: string) {
		if (!settings) return;
		const window = Math.round(Number(value));
		if (!Number.isFinite(window) || window <= 0) return;
		patch({ compaction: { ...settings.compaction, window } });
	}

	function label(model: ModelChoice): string {
		const provider = settings?.providers.find((provider) => provider.id === model.provider);
		return `${provider?.name ?? 'Unknown'}/${model.name}`;
	}
</script>

{#snippet row(title: string, description: string)}
	<span class="min-w-0 flex-1">
		<span class="block text-title font-medium">{title}</span>
		<span class="block text-label text-secondary">{description}</span>
	</span>
{/snippet}

{#if settings}
	{@const compaction = settings.compaction}
	{@const subagents = settings.subagents}
	<div class="mx-auto flex w-full max-w-170 flex-col gap-6 px-6 py-6">
		<section>
			<div class="pb-3 pl-1">
				<h2 class="text-title font-medium">Subagents</h2>
				<p class="text-secondary">Agents the main agent starts to hand off work</p>
			</div>

			<div class="flex flex-col rounded-surface surface p-1">
				<div class="flex items-center gap-3 px-3 py-2.5">
					{@render row('Allow subagents', 'Give agents the spawn_agent tool')}
					<Switch
						checked={subagents.enabled}
						label="Allow subagents"
						onchange={(enabled) => setSubagents({ enabled })}
					/>
				</div>
				<div class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'Default model',
						'What a subagent runs on unless the agent picks a model, with its reasoning effort'
					)}
					{#if subagents.model || subagents.effort}
						<button
							class="icon-button"
							onclick={() => setSubagents({ model: null, effort: null })}
							{@attach tooltip('Use the model of the agent that starts it')}
						>
							<ArrowCounterClockwiseIcon size={14} />
						</button>
					{/if}
					<ModelPicker
						selection={{ model: subagents.model, effort: subagents.effort }}
						onchange={({ model, effort }) => setSubagents({ model, effort })}
						placeholder="Same as its parent"
						placement="bottom-end"
						starrable={false}
					/>
				</div>
				<label class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'Nesting depth',
						'How many levels of subagents can start subagents of their own, up to 5'
					)}
					<input
						class="input w-16 flex-none text-right"
						type="number"
						min="1"
						max="5"
						value={subagents.depth}
						onchange={(event) => setDepth(event.currentTarget.value)}
					/>
				</label>
				<div class="flex flex-col gap-2 px-3 py-2.5">
					<div class="flex items-center gap-3">
						{@render row(
							'Models agents can choose',
							'Offered to agents by name when they start a subagent, from any provider'
						)}
						<ModelPicker
							selection={{ model: null, effort: null }}
							onchange={({ model }) =>
								model && setSubagents({ models: [...subagents.models, model] })}
							placeholder="Add a model"
							placement="bottom-end"
							effort={false}
							starrable={false}
						/>
					</div>
					{#if subagents.models.length}
						<div class="flex flex-wrap gap-1.5">
							{#each subagents.models as model (label(model))}
								<span
									class="flex h-6.5 items-center gap-1 rounded-control bg-alternate-base pr-1 pl-2 text-label"
								>
									<span class="truncate">{label(model)}</span>
									<button
										class="icon-button size-5 text-secondary hover:text-window-text"
										onclick={() =>
											setSubagents({
												models: subagents.models.filter(
													(other) => other !== model
												)
											})}
										aria-label="Remove {label(model)}"
									>
										<XIcon size={10} />
									</button>
								</span>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</section>

		<section>
			<div class="pb-3 pl-1">
				<h2 class="text-title font-medium">Compaction</h2>
				<p class="text-secondary">
					Replacing a long conversation with a summary when the context fills up
				</p>
			</div>

			<div class="flex flex-col rounded-surface surface p-1">
				<div class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'Compact automatically',
						'Summarize the conversation once 80% of the context is used'
					)}
					<Switch
						checked={compaction.auto}
						label="Compact automatically"
						onchange={(auto) => patch({ compaction: { ...compaction, auto } })}
					/>
				</div>
				<div class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'Let agents compact',
						'Give agents the compact tool, to summarize the conversation themselves when it saves context'
					)}
					<Switch
						checked={compaction.tool}
						label="Let agents compact"
						onchange={(tool) => patch({ compaction: { ...compaction, tool } })}
					/>
				</div>
				<label class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'Fallback context window',
						"Tokens to assume when a provider does not report a model's context size"
					)}
					<input
						class="input w-28 flex-none text-right"
						type="number"
						min="1000"
						step="1000"
						value={compaction.window}
						onchange={(event) => setWindow(event.currentTarget.value)}
					/>
				</label>
			</div>
		</section>

		<section>
			<div class="pb-3 pl-1">
				<h2 class="text-title font-medium">Web search</h2>
				<p class="text-secondary">Where agents look things up with the web_search tool</p>
			</div>

			<div class="flex flex-col rounded-surface surface p-1">
				<label class="flex items-center gap-3 px-3 py-2.5">
					{@render row(
						'SearXNG instance',
						'An instance that serves json, needed for agents to search'
					)}
					<input
						class="input w-64 flex-none"
						type="url"
						placeholder="https://searx.example.com"
						spellcheck="false"
						value={settings.search.searxng ?? ''}
						onchange={(event) =>
							patch({ search: { searxng: event.currentTarget.value } })}
					/>
				</label>
			</div>
		</section>

		<section>
			<div class="pb-3 pl-1">
				<h2 class="text-title font-medium">System prompts</h2>
				<p class="text-secondary">What every agent is told before a conversation starts</p>
			</div>

			<div class="flex flex-col gap-3">
				{#each prompts as { kind, title, description } (kind)}
					{@const prompt = settings.prompts[kind]}
					<div class="flex flex-col gap-2.5 rounded-surface surface p-3">
						<div class="flex items-center gap-3 pl-1">
							{@render row(title, description)}
							<button
								class="icon-button"
								disabled={prompt.text === prompt.default}
								onclick={() => setPrompt(kind, null)}
								{@attach tooltip('Reset to the default')}
							>
								<ArrowCounterClockwiseIcon size={14} />
							</button>
						</div>
						<textarea
							class="textarea"
							value={prompt.text}
							onchange={(event) => setPrompt(kind, event.currentTarget.value)}
							spellcheck="false"></textarea>
					</div>
				{/each}
			</div>
		</section>

		{#if failure}
			<div class="alert whitespace-pre-wrap">
				<WarningCircleIcon size={14} />{failure}
			</div>
		{/if}
	</div>
{/if}
