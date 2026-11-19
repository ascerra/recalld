<!-- Detail panel for the selected memory (DESIGN §2, §8). Reads are
     read-only; Reinforce and Forget are the only writes, Forget confirms. -->
<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query';
  import { AlertDialog, DropdownMenu } from 'bits-ui';
  import Icon from './Icon.svelte';
  import DecayCurve from './DecayCurve.svelte';
  import { forget, memoryHistory, memoryRecord, memoryRows, reinforce } from '$lib/api/memories';
  import { ago, duration, phaseName } from '$lib/format';
  import { DAY_MS, strengthAt } from '$lib/fsrs';

  let {
    id,
    onclose,
    secondary,
  }: {
    id: string;
    onclose: () => void;
    /** "Show on map" in Explore, "Local graph" on the map. */
    secondary: { label: string; onclick: () => void };
  } = $props();

  const client = useQueryClient();
  const row = createQuery(() => ({
    queryKey: ['memory-row', id],
    queryFn: async () => (await memoryRows([id]))[0] ?? null,
  }));
  const record = createQuery(() => ({ queryKey: ['memory', id], queryFn: () => memoryRecord(id) }));
  const history = createQuery(() => ({ queryKey: ['history', id], queryFn: () => memoryHistory(id, 3) }));

  let showFull = $state(false);
  let previous = $state<number | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let confirming = $state(false);

  const r = $derived(row.data ? strengthAt(row.data.lastAccessedAt, row.data.stability) : 0);
  const elapsed = $derived(row.data ? Math.max(0, Date.now() - row.data.lastAccessedAt) / DAY_MS : 0);
  const labels = $derived(
    [...(row.data?.entities ?? []), ...(row.data?.tags ?? [])].slice(0, 3).join(' · '),
  );
  const KINDS: Record<string, string> = {
    direct: 'recalled',
    associative: 'recalled through a link',
    reinforce: 'reinforced',
    sweep: 'checked by a decay sweep',
  };

  async function doReinforce(rating: number) {
    if (!row.data) return;
    busy = true;
    error = null;
    previous = row.data.stability;
    try {
      await reinforce(id, row.data.namespace, rating);
      await client.invalidateQueries();
    } catch (e) {
      error = (e as Error).message;
      previous = null;
    } finally {
      busy = false;
    }
  }

  async function doForget() {
    if (!row.data) return;
    busy = true;
    try {
      await forget(id, row.data.namespace);
      confirming = false;
      await client.invalidateQueries();
      onclose();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<aside class="panel" aria-label="Selected memory">
  {#if row.isPending}
    <div class="skeleton line"></div>
    <div class="skeleton title"></div>
    <div class="skeleton block"></div>
  {:else if !row.data}
    <p class="muted">This memory no longer exists.</p>
  {:else}
    {@const m = row.data}
    <div class="head">
      <div class="top">
        <span class="status mono"><span class="dot"></span>{phaseName(m.phase)} · accessed {ago(m.lastAccessedAt)}</span>
        <button type="button" class="icon-btn close" aria-label="Close details" onclick={onclose}><Icon name="close" stroke={1.8} /></button>
      </div>
      <h2 class:ghost={m.summary === null}>{m.summary ?? 'Summary dropped'}</h2>
      <span class="meta">{m.namespace}{labels ? ` · ${labels}` : ''} · {m.degree} links</span>
      {#if m.phase === 1 && record.data?.fullText}
        <button type="button" class="link" aria-expanded={showFull} onclick={() => (showFull = !showFull)}>
          {showFull ? 'Hide full text' : 'Show full text'}
        </button>
        {#if showFull}<p class="full">{record.data.fullText}</p>{/if}
      {/if}
    </div>

    <figure>
      <div class="fig-head">
        <span class="card-title">Retrievability</span>
        <span class="mono values">R {r.toFixed(2)} · S {duration(m.stability)}{m.permastore ? ' · permastore' : ''}</span>
      </div>
      <DecayCurve stability={m.stability} elapsedDays={elapsed} previousStability={previous} />
      <figcaption>
        {m.nextChange ? `${phaseName(m.nextChange.to)} ${m.nextChange.at <= Date.now() ? 'at the next sweep' : `in ${duration((m.nextChange.at - Date.now()) / DAY_MS)}`}` : m.permastore ? 'In permastore' : 'No change expected'}
        · created {new Date(m.createdAt).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}
      </figcaption>
    </figure>

    <section class="activity" aria-label="Recent activity">
      <span class="micro-label">Activity</span>
      {#each history.data?.events ?? [] as e (e.at + e.kind)}
        <div class="act"><span class="mono t">{ago(e.at)}</span><span>{e.agent ?? 'unknown agent'} <span class="muted">{KINDS[e.kind] ?? e.kind}</span></span></div>
      {:else}
        <span class="muted small">No access recorded yet.</span>
      {/each}
    </section>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="actions">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger class="btn-primary grow" disabled={busy}>Reinforce</DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content class="menu" sideOffset={6} align="start">
            {#each [[4, 'Easy'], [3, 'Good'], [2, 'Hard'], [1, 'Again']] as [q, label] (q)}
              <DropdownMenu.Item class="menu-item" onSelect={() => doReinforce(q as number)}>{label}</DropdownMenu.Item>
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      <button type="button" class="btn-secondary grow" onclick={secondary.onclick}>{secondary.label}</button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger class="btn-secondary square" aria-label="More actions"><Icon name="more" /></DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content class="menu" sideOffset={6} align="end">
            <DropdownMenu.Item class="menu-item" onSelect={() => (confirming = true)}>Forget…</DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>

    <AlertDialog.Root bind:open={confirming}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay class="dialog-scrim" />
        <AlertDialog.Content class="dialog">
          <AlertDialog.Title class="card-title">Forget this memory?</AlertDialog.Title>
          <AlertDialog.Description class="dialog-text">
            “{m.summary ?? 'Summary dropped'}” becomes a tombstone: its content is removed and it no longer appears in recall.
          </AlertDialog.Description>
          <div class="dialog-actions">
            <AlertDialog.Cancel class="btn-secondary">Keep it</AlertDialog.Cancel>
            <AlertDialog.Action class="btn-primary" disabled={busy} onclick={doForget}>Forget</AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  {/if}
</aside>

<style>
  .panel {
    flex: 0 1 var(--panel-width);
    min-width: var(--panel-min);
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--space-7);
    padding: var(--space-6) var(--space-7) var(--space-7);
    background: var(--color-surface);
    border-left: var(--hairline) solid var(--color-border);
    overflow: auto;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .close {
    margin-right: calc(var(--space-2) * -1);
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-status);
    color: var(--color-accent-text);
  }
  .dot {
    width: var(--dot);
    height: var(--dot);
    border-radius: var(--radius-pill);
    background: var(--color-accent);
  }
  h2 {
    margin: 0;
    font: var(--weight-regular) var(--text-memory) / 1.2 var(--font-serif);
    letter-spacing: -0.01em;
    text-wrap: pretty;
  }
  h2.ghost {
    font-style: italic;
    color: var(--color-text-muted);
  }
  .meta,
  figcaption,
  .small {
    font-size: var(--text-control);
    color: var(--color-text-muted);
  }
  .link {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-accent-text);
    font-size: var(--text-control);
    text-decoration: underline;
    text-underline-offset: 0.2em;
    cursor: pointer;
  }
  .full {
    margin: 0;
    white-space: pre-wrap;
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .fig-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .values {
    font-size: var(--text-status);
    color: var(--color-text-muted);
  }
  .activity {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .act {
    display: flex;
    gap: var(--space-3);
    font-size: var(--text-control);
  }
  .t {
    flex: 0 0 auto;
    min-width: var(--space-8);
    font-size: var(--text-status);
    color: var(--color-text-muted);
  }
  .muted {
    color: var(--color-text-muted);
  }
  .error {
    margin: 0;
    font-size: var(--text-control);
    color: var(--color-text);
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    margin-top: auto;
  }
  .actions :global(.grow) {
    flex: 1 1 auto;
  }
  .actions :global(.square) {
    width: var(--button-h);
    padding: 0;
    color: var(--color-text-2);
  }
  .skeleton.line {
    height: var(--space-3);
    width: 60%;
  }
  .skeleton.title {
    height: var(--space-8);
  }
  .skeleton.block {
    height: var(--space-8);
    flex: 0 0 calc(var(--space-8) * 3);
  }
  :global(.dialog-scrim) {
    position: fixed;
    inset: 0;
    background: var(--color-scrim);
    z-index: 50;
  }
  :global(.dialog) {
    position: fixed;
    top: var(--palette-top);
    left: 50%;
    transform: translateX(-50%);
    width: min(calc(var(--panel-width) + var(--space-8) * 2), calc(100vw - var(--space-8)));
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--card-pad);
    background: var(--color-surface);
    border: var(--hairline) solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-popover);
    z-index: 51;
  }
  :global(.dialog-text) {
    margin: 0;
    color: var(--color-text-2);
  }
  :global(.dialog-actions) {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
  @media (max-width: 900px) {
    .panel {
      flex-basis: var(--panel-min);
    }
  }
  @media (max-width: 600px) {
    .panel {
      min-width: 0;
      height: auto;
      border-left: 0;
      border-top: var(--hairline) solid var(--color-border);
    }
  }
</style>
