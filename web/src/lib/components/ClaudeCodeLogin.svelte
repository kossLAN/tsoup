<script lang="ts">
	import { finishClaudeCodeLogin, startClaudeCodeLogin, type ClaudeCodeAccount } from '$lib/api';
	import { ArrowSquareOutIcon } from '$lib/icons';

	let { account = $bindable() }: { account: ClaudeCodeAccount | null } = $props();

	let url = $state<string | null>(null);
	let code = $state('');
	let busy = $state(false);
	let failure = $state<string | null>(null);

	async function attempt(action: () => Promise<void>) {
		busy = true;
		failure = null;
		try {
			await action();
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}

	function start() {
		attempt(async () => {
			url = (await startClaudeCodeLogin()).url;
			code = '';
		});
	}

	function finish() {
		attempt(async () => {
			account = await finishClaudeCodeLogin(code);
			url = null;
			code = '';
		});
	}

	function keydown(event: KeyboardEvent) {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		if (code.trim() && !busy) finish();
	}
</script>

<div class="field">
	<span class="w-20 shrink-0 text-secondary">Account</span>
	<span class="flex min-w-0 flex-1 items-center gap-2">
		{#if account === null}
			<span class="text-secondary">Checking</span>
		{:else if account.signed_in}
			<span class="truncate">{account.email}</span>
			{#if account.subscription}
				<span class="shrink-0 text-label text-secondary">{account.subscription}</span>
			{/if}
		{:else}
			<span class="text-secondary">Not signed in</span>
		{/if}
	</span>
	<button
		type="button"
		class="button enabled:hover:bg-window-text/8"
		disabled={busy}
		onclick={start}
	>
		{account?.signed_in ? 'Switch account' : 'Sign in'}
	</button>
</div>
{#if url}
	<div class="field">
		<span class="w-20 shrink-0"></span>
		<a
			class="-ml-3 button gap-1.5 text-accent hover:bg-window-text/8"
			href={url}
			target="_blank"
			rel="noreferrer"
		>
			Open the Claude sign-in page
			<ArrowSquareOutIcon size={12} />
		</a>
	</div>
	<label class="field">
		<span class="w-20 shrink-0 text-secondary">Code</span>
		<input
			class="input"
			bind:value={code}
			placeholder="Paste the code shown after signing in"
			spellcheck="false"
			autocomplete="off"
			onkeydown={keydown}
		/>
		<button
			type="button"
			class="button enabled:hover:bg-window-text/8"
			disabled={busy || !code.trim()}
			onclick={finish}
		>
			Finish
		</button>
	</label>
{/if}
{#if failure}
	<p class="px-1.5 pb-1 text-label text-danger">{failure}</p>
{/if}
