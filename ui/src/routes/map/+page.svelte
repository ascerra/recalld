<script lang="ts">
  import { page } from '$app/state';
  import { createQuery } from '@tanstack/svelte-query';
  import Screen from '$lib/components/Screen.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import ScopeChip from '$lib/components/ScopeChip.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import MemoryPanel from '$lib/components/MemoryPanel.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { scope } from '$lib/scope.svelte';
  import { selection } from '$lib/selection.svelte';
  import { fetchGraph } from '$lib/api/snapshot';
  import { phaseName, truncateWords, count } from '$lib/format';
  import { EDGE_LABELS, EDGE } from '$lib/map/encoding';
  import { memoryRows } from '$lib/api/memories';
  import { live } from '$lib/live.svelte';
  import { ago } from '$lib/format';
  import { simulate } from '$lib/map/simulator';

  $effect(() => {
    live.start();
    return () => live.release();
  });
  // Dev simulator: /map?simulate=5 emits five synthetic recalls a second.
  $effect(() => {
    const rate = Number(page.url.searchParams.get('simulate') ?? 0);
    if (rate > 0 && graph.data) return simulate(graph.data, rate);
  });
  let tick = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (tick = Date.now()), 15_000);
    return () => clearInterval(t);
  });
  const pill = $derived.by(() => {
    tick;
    const r = live.latest;
    if (!r) return null;
    const e = r.event;
    if (e.kind === 'lagged') return null;
    const who = 'agent' in e && e.agent ? e.agent : e.kind === 'sweep' || e.kind === 'phase_change' ? 'recalld' : 'unknown agent';
    const verb = e.kind === 'phase_change' ? 'decay' : e.kind === 'sweep' ? 'decay sweep' : e.kind;
    return { who, verb, when: ago(e.at), recent: Date.now() - e.at < 60_000, event: r };
  });

  const graph = createQuery(() => ({
    queryKey: ['graph', scope.current],
    queryFn: () => fetchGraph(scope.current),
    staleTime: 60_000,
  }));

  let phases = $state(new Set([1, 2, 3]));
  const togglePhase = (p: number) => {
    const next = new Set(phases);
    if (next.has(p)) next.delete(p);
    else next.add(p);
    phases = next;
  };
  // Temporal links are hidden by default (DESIGN §6).
  let linkSel = $state<string[]>(['1', '2', '3', '4', '5', '7']);
  const edgeTypes = $derived(new Set(linkSel.map(Number)));
  const presentTypes = $derived.by(() => {
    const t = graph.data?.type;
    if (!t) return [] as number[];
    const seen = new Set<number>();
    for (let i = 0; i < t.length; i++) seen.add(t[i]);
    return [...seen].sort();
  });
  const shownTypes = $derived(presentTypes.filter((t) => edgeTypes.has(t)).length);

  let days = $state(0);
  const daysLabel = $derived(days === 0 ? 'now' : days < 60 ? `+${days} d` : `+${Math.round(days / 30)} mo`);

  let map: { zoomIn(): void; zoomOut(): void; fit(): void; focusNeighborhood(id: string): void; center(id: string): void } | undefined = $state();
  let panelOpen = $state(true);

  // ?id=… (from Explore's "Show on map" or the palette's ⌘↵).
  $effect(() => {
    const id = page.url.searchParams.get('id');
    if (id) {
      selection.select(id);
      panelOpen = true;
      setTimeout(() => map?.center(id), 300);
    }
  });

  const selectedRow = createQuery(() => ({
    queryKey: ['memory-row', selection.id],
    queryFn: async () => (selection.id ? ((await memoryRows([selection.id]))[0] ?? null) : null),
    enabled: !!selection.id,
  }));
</script>

<Screen>
  <PageHeader title="Memory map" meta={graph.data ? `${count(graph.data.ids.length)} memories` : undefined} />

  <div role="toolbar" aria-label="Graph filters" class="toolbar">
    <ScopeChip />
    <span class="toolbar-divider"></span>
    {#each [1, 2, 3] as p (p)}
      <button type="button" class="chip" aria-pressed={phases.has(p)} onclick={() => togglePhase(p)}>{phaseName(p)}</button>
    {/each}
    <FilterMenu
      label="Links · {shownTypes} {shownTypes === 1 ? 'type' : 'types'}"
      options={presentTypes.map((t) => ({ value: String(t), label: EDGE_LABELS[t] }))}
      bind:selected={linkSel}
      showCount={false}
    />
    <div class="scrubber">
      <label for="decay-scrub">Decay</label>
      <input id="decay-scrub" type="range" min="0" max="365" step="1" bind:value={days} aria-valuetext={daysLabel} />
      <span class="mono when">{daysLabel}</span>
    </div>
  </div>

  <div class="canvas-card">
    {#if graph.isPending}
      <div class="state"><span class="skeleton blob"></span></div>
    {:else if graph.isError}
      <div class="state muted">The map could not load: {graph.error.message}</div>
    {:else if graph.data && graph.data.ids.length === 0}
      <div class="state muted">No memories in this scope yet. Agents add them with store_memory.</div>
    {:else if graph.data}
      {#if pill}
        <button type="button" class="pill" class:quiet={!pill.recent} title="Replay in slow motion" onclick={() => live.replay(pill.event)}>
          <span class="pill-dot"></span>{pill.who} · {pill.verb} · {pill.when}
        </button>
      {:else}
        <span class="pill quiet"><span class="pill-dot idle"></span>{live.connected ? 'listening for agents' : 'connecting to the daemon'}</span>
      {/if}
      {#key graph.data}
        {#await import('$lib/map/MemoryMap.svelte') then { default: MemoryMap }}
          <MemoryMap
            bind:this={map}
            snapshot={graph.data}
            scopeKey={scope.current}
            selectedId={selection.id}
            projectionDays={days}
            {phases}
            {edgeTypes}
            onselect={(id) => {
              selection.select(id);
              if (id) panelOpen = true;
            }}
          />
        {/await}
      {/key}
      <div class="legend" aria-label="Legend">
        <span><span class="sw full"></span>Full</span>
        <span><span class="sw summary"></span>Summary</span>
        <span><span class="sw ghost"></span>Ghost</span>
        <span><span class="sw lit"></span>Recalled</span>
        <span><span class="sw suppressed"></span>Suppressed</span>
      </div>
      <div class="zoom">
        <button type="button" class="zbtn" aria-label="Zoom in" onclick={() => map?.zoomIn()}><Icon name="plus" stroke={1.8} /></button>
        <button type="button" class="zbtn" aria-label="Zoom out" onclick={() => map?.zoomOut()}><Icon name="minus" stroke={1.8} /></button>
        <button type="button" class="zbtn" aria-label="Fit graph to view" onclick={() => map?.fit()}><Icon name="fit" stroke={1.8} /></button>
      </div>
    {/if}
  </div>
</Screen>

{#if selection.id}
  {#if panelOpen}
    <div class="panel-wrap">
      <button type="button" class="icon-btn collapse-panel" aria-label="Collapse details" onclick={() => (panelOpen = false)}><Icon name="expand" stroke={1.8} /></button>
      <MemoryPanel id={selection.id} onclose={() => selection.select(null)}
        secondary={{ label: 'Local graph', onclick: () => map?.focusNeighborhood(selection.id!) }} />
    </div>
  {:else}
    <aside class="rail" aria-label="Selected memory, collapsed">
      <button type="button" class="rail-btn" aria-label="Expand details" aria-expanded="false" onclick={() => (panelOpen = true)}><Icon name="collapse" stroke={1.8} /></button>
      <span class="live-dot"></span>
      <span class="mono vertical">{truncateWords(selectedRow.data?.summary ?? 'Summary dropped', 22)}</span>
    </aside>
  {/if}
{/if}

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .scrubber {
    display: flex;
    align-items: center;
    gap: calc(var(--space-2) + var(--space-1) / 2);
    margin-left: auto;
  }
  .scrubber label {
    font-size: var(--text-control);
    color: var(--color-text-muted);
  }
  .scrubber input {
    width: calc(var(--space-8) * 3.75);
    accent-color: var(--color-accent);
  }
  .when {
    font-size: var(--text-explain);
    min-width: var(--space-8);
  }
  .canvas-card {
    position: relative;
    flex: 1 1 auto;
    min-height: calc(var(--palette-top) * 4);
    border: var(--hairline) solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-canvas);
    overflow: hidden;
  }
  .state {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8);
  }
  .muted {
    color: var(--color-text-muted);
  }
  .skeleton.blob {
    width: 40%;
    height: 40%;
    border-radius: var(--radius-lg);
  }
  .pill {
    position: absolute;
    left: calc(var(--space-3) + var(--space-1) / 2);
    top: calc(var(--space-3) + var(--space-1) / 2);
    z-index: 4;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: calc(var(--control-h) - var(--space-1));
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--color-surface);
    border: var(--hairline) solid var(--color-border);
    font-size: var(--text-small);
    color: var(--color-accent-strong);
    cursor: pointer;
  }
  .pill.quiet {
    color: var(--color-text-muted);
    cursor: default;
  }
  .pill-dot {
    width: calc(var(--dot) + var(--hairline));
    height: calc(var(--dot) + var(--hairline));
    border-radius: var(--radius-pill);
    background: var(--color-accent);
    box-shadow: 0 0 0 calc(var(--hairline) * 3) var(--color-accent-soft-border);
  }
  .pill-dot.idle {
    background: var(--color-border-strong);
    box-shadow: none;
  }
  .legend {
    position: absolute;
    left: calc(var(--space-3) + var(--space-1) / 2);
    bottom: calc(var(--space-3) + var(--space-1) / 2);
    z-index: 4;
    display: flex;
    flex-wrap: wrap;
    gap: calc(var(--space-3) + var(--space-1) / 2);
    font-size: var(--text-small);
    color: var(--color-text-muted);
    pointer-events: none;
  }
  .legend > span {
    display: flex;
    align-items: center;
    gap: calc(var(--space-1) * 1.5);
  }
  .sw {
    width: var(--swatch);
    height: var(--swatch);
    border-radius: var(--radius-pill);
  }
  .sw.full {
    background: var(--color-node);
  }
  .sw.summary {
    background: var(--color-node);
    opacity: 0.38;
  }
  .sw.ghost {
    border: var(--hairline) solid var(--color-node);
  }
  .sw.lit {
    background: var(--color-node-lit);
  }
  .sw.suppressed {
    background: var(--color-suppressed);
  }
  .zoom {
    position: absolute;
    right: calc(var(--space-3) + var(--space-1) / 2);
    bottom: calc(var(--space-3) + var(--space-1) / 2);
    z-index: 4;
    display: flex;
    gap: var(--space-1);
  }
  .zbtn {
    width: var(--icon-btn);
    height: var(--icon-btn);
    display: flex;
    align-items: center;
    justify-content: center;
    border: var(--hairline) solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface);
    color: var(--color-text);
    cursor: pointer;
  }
  .panel-wrap {
    position: relative;
    display: flex;
    height: 100%;
  }
  .collapse-panel {
    position: absolute;
    left: calc(var(--space-5) * -1);
    top: var(--space-6);
    z-index: 5;
    width: var(--space-6);
    height: var(--space-6);
    background: var(--color-surface);
    border: var(--hairline) solid var(--color-border);
  }
  .rail {
    flex: 0 0 var(--panel-rail);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--space-4) + var(--space-1) / 2);
    padding: var(--space-5) 0;
    background: var(--color-surface);
    border-left: var(--hairline) solid var(--color-border);
  }
  .rail-btn {
    width: var(--nav-item-h);
    height: var(--nav-item-h);
    display: flex;
    align-items: center;
    justify-content: center;
    border: var(--hairline) solid var(--color-border);
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-2);
    cursor: pointer;
  }
  .live-dot {
    width: var(--dot);
    height: var(--dot);
    border-radius: var(--radius-pill);
    background: var(--color-accent);
    box-shadow: 0 0 0 calc(var(--hairline) * 3) var(--color-accent-soft-border);
  }
  .vertical {
    writing-mode: vertical-rl;
    font-size: var(--text-status);
    letter-spacing: 0.04em;
    color: var(--color-text-2);
  }
</style>
