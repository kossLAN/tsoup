<script lang="ts">
	import type { Image } from '$lib/api';
	import { ArrowElbowDownLeftIcon, PlusIcon, StopIcon, XIcon } from '$lib/icons';
	import { tooltip } from '$lib/popup';
	import type { Snippet } from 'svelte';

	let {
		busy = false,
		onsend,
		onstop,
		accessory,
		placeholder = 'Ask the agent',
		dense = false
	}: {
		busy?: boolean;
		onsend: (text: string, images: Image[]) => Promise<boolean>;
		onstop?: () => void;
		accessory?: Snippet;
		placeholder?: string;
		dense?: boolean;
	} = $props();

	const types = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

	let text = $state('');
	let images = $state<Image[]>([]);
	let sending = $state(false);
	let input: HTMLTextAreaElement;
	let picker: HTMLInputElement;

	const ready = $derived(Boolean(text.trim()) || images.length > 0);

	$effect(() => {
		void text;
		input.style.height = 'auto';
		input.style.height = `${input.scrollHeight}px`;
	});

	async function send() {
		const content = text.trim();
		const attached = images;
		if (!ready || sending) return;
		sending = true;
		text = '';
		images = [];
		if (!(await onsend(content, attached)) && !text && !images.length) {
			text = content;
			images = attached;
		}
		sending = false;
	}

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			send();
		}
	}

	function read(file: File): Promise<Image> {
		return new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onerror = () => reject(reader.error);
			reader.onload = () =>
				resolve({ media_type: file.type, data: String(reader.result).split(',')[1] ?? '' });
			reader.readAsDataURL(file);
		});
	}

	async function attach(files: Iterable<File>) {
		const accepted = Array.from(files).filter((file) => types.includes(file.type));
		images.push(...(await Promise.all(accepted.map(read))));
	}

	function paste(event: ClipboardEvent) {
		const files = Array.from(event.clipboardData?.files ?? []);
		if (!files.some((file) => types.includes(file.type))) return;
		event.preventDefault();
		attach(files);
	}

	function drop(event: DragEvent) {
		event.preventDefault();
		attach(event.dataTransfer?.files ?? []);
	}

	function picked() {
		attach(picker.files ?? []);
		picker.value = '';
	}
</script>

<div class={['mx-auto w-full max-w-chat', dense ? 'px-1.5 pb-1.5' : 'px-6 pb-3']}>
	<div
		class="flex flex-col gap-2 rounded-surface surface py-1.5 pr-1.5 pl-1.5 transition-colors focus-within:border-accent"
		role="presentation"
		ondragover={(event) => event.preventDefault()}
		ondrop={drop}
	>
		{#if images.length}
			<div class="flex flex-wrap gap-1.5 pt-1 pl-1">
				{#each images as image, index (image)}
					<div class="group relative">
						<img
							class="size-14 rounded-control border border-light object-cover"
							src="data:{image.media_type};base64,{image.data}"
							alt="Attachment {index + 1}"
						/>
						<button
							class="absolute -top-1 -right-1 inline-flex size-4 items-center justify-center rounded-full surface opacity-0 transition-opacity group-hover:opacity-100"
							onclick={() => images.splice(index, 1)}
							aria-label="Remove attachment {index + 1}"
						>
							<XIcon size={10} />
						</button>
					</div>
				{/each}
			</div>
		{/if}
		<div class="flex items-end gap-1.5">
			<button
				class="icon-button text-secondary hover:text-window-text"
				onclick={() => picker.click()}
				{@attach tooltip('Attach images')}
			>
				<PlusIcon size={16} />
			</button>
			<textarea
				class="max-h-60 flex-1 resize-none overflow-y-auto py-0.75 text-title/5 outline-none placeholder:text-secondary"
				bind:this={input}
				bind:value={text}
				onkeydown={keydown}
				onpaste={paste}
				rows="1"
				{placeholder}></textarea>
			<input
				class="hidden"
				type="file"
				accept={types.join(',')}
				multiple
				bind:this={picker}
				onchange={picked}
			/>
			{#if busy && onstop}
				<button class="icon-button" onclick={onstop} {@attach tooltip('Stop')}>
					<StopIcon size={16} />
				</button>
			{/if}
			{#if !busy || !onstop || ready}
				<button
					class="icon-button"
					disabled={!ready || sending}
					onclick={send}
					{@attach tooltip(busy ? 'Queue' : 'Send')}
				>
					<ArrowElbowDownLeftIcon size={16} />
				</button>
			{/if}
		</div>
	</div>
	{#if accessory}
		<div class="flex items-center justify-end gap-1 px-1 pt-1">
			{@render accessory()}
		</div>
	{/if}
</div>
