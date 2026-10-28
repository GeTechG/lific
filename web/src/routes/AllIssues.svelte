<script lang="ts">
  // Every issue of every project the caller can see, as a list (`/issues`)
  // or a board (`/board`). No new endpoint: `GET /issues` without a
  // project_id is already filtered to the visible projects (LIF-197), the
  // same call Home's "My active issues" makes.
  //
  // Deliberately not IssueList: that one is bound to a single project's read
  // model, labels, modules and saved views. This is the overview; filtering,
  // bulk actions and swimlanes stay on the project's own list.
  import { listIssues, updateIssue, type Issue } from "../lib/api";
  import { STATUSES, TERMINAL_STATUSES } from "../lib/issues/grouping";
  import { compareIssues, defaultSortDir, type SortField, type SortDir } from "../lib/issues/sort";
  import { loadListState, saveListState } from "../lib/issues/persistence";
  import { startAutoRefresh } from "../lib/autoRefresh.svelte";
  import { motionReduced } from "../lib/theme";
  import StatusIcon, { statusLabel } from "../lib/StatusIcon.svelte";
  import PriorityIcon from "../lib/PriorityIcon.svelte";
  import IssueCard from "../lib/issues/IssueCard.svelte";
  import AssigneeMarker from "../lib/issues/AssigneeMarker.svelte";
  import ErrorState from "../lib/ErrorState.svelte";
  import { toast } from "../lib/toast/toast.svelte";
  import { List, LayoutGrid, ArrowUp, ArrowDown, Signal, Clock, History, Hash } from "lucide-svelte";
  import { dndzone, type DndEvent } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import { getContext } from "svelte";

  let {
    navigate,
    layout = "list",
  }: {
    navigate: (path: string) => void;
    layout?: "list" | "board";
  } = $props();

  const topbarCtx = getContext<{
    set: (s: import("svelte").Snippet | undefined) => void;
  } | undefined>("lific:topbar");
  $effect(() => {
    topbarCtx?.set(topbarContent);
    return () => topbarCtx?.set(undefined);
  });

  // Keyed by status. The board's drop zones write into it while a card is
  // dragged, so it is state rather than a derivation of a flat list.
  let columns = $state<Record<string, Issue[]>>({});
  let loading = $state(true);
  let error = $state("");
  let dragging = $state(false);
  let total = $derived(STATUSES.reduce((n, s) => n + (columns[s]?.length ?? 0), 0));

  // Same ordering as a project's list and board, remembered under a key no
  // numeric project id can take.
  const SORT_LABELS: Record<SortField, string> = { priority: "Priority", age: "Age", updated: "Updated", number: "Issue #" };
  const saved = loadListState("all");
  let sortField = $state<SortField>(saved.sortField ?? "priority");
  let sortDir = $state<SortDir>(saved.sortDir ?? "asc");
  let sortOpen = $state(false);

  const sorted = (items: Issue[]) =>
    [...items].sort((a, b) => compareIssues(a, b, { searchQuery: "", scores: new Map(), sortField, sortDir }));

  function selectSort(field: SortField) {
    if (sortField === field) sortDir = sortDir === "asc" ? "desc" : "asc";
    else {
      sortField = field;
      sortDir = defaultSortDir(field);
    }
    saveListState("all", { sortField, sortDir });
    columns = Object.fromEntries(STATUSES.map((s) => [s, sorted(columns[s] ?? [])]));
  }

  const open = (i: Issue) => navigate(`/${i.identifier.replace(/-\d+$/, "")}/issues/${i.identifier}`);
  const flipMs = () => (motionReduced() ? 0 : 150);

  async function load(initial = true) {
    // One call per status: the endpoint has no OR filter and caps a page at
    // 500, so a single unfiltered call would let old done issues crowd out
    // live ones.
    // ponytail: at most 500 per status, newest first; paginate if a column outgrows it.
    const res = await Promise.all(STATUSES.map((status) => listIssues({ status, limit: 500 })));
    const failed = res.find((r) => !r.ok);
    if (failed && !failed.ok) {
      if (initial) error = failed.error;
    } else {
      error = "";
      columns = Object.fromEntries(STATUSES.map((s, n) => [s, res[n].ok ? sorted(res[n].data) : []]));
    }
    loading = false;
  }

  $effect(() => {
    load();
    return startAutoRefresh({
      refresh: () => load(false),
      isBusy: () => loading || dragging,
      realtimeDebounceMs: 750,
      realtimeMaxWaitMs: 5_000,
      shouldRefresh: (event) =>
        event.type === "resync.required" ||
        event.type.startsWith("project.") ||
        event.type.startsWith("issue."),
    });
  });

  function consider(status: string, e: CustomEvent<DndEvent<Issue>>) {
    dragging = true;
    columns[status] = e.detail.items;
  }

  async function finalize(status: string, e: CustomEvent<DndEvent<Issue>>) {
    columns[status] = e.detail.items;
    // The dropped card still carries its old status: that is the one that moved.
    const moved = e.detail.items.find((i) => i.status !== status);
    if (moved) {
      const res = await updateIssue(moved.id, { status });
      if (res.ok) moved.status = status;
      else toast(res.error, { kind: "error" }); // e.g. a viewer of that project; the reload below puts the card back
    }
    dragging = false;
    if (moved) load(false);
  }
</script>

<svelte:window
  onclick={() => (sortOpen = false)}
  onkeydown={(e) => { if (e.key === "Escape") sortOpen = false; }}
/>

{#snippet topbarContent()}
  <div class="flex items-center gap-3 px-6 py-2 w-full">
    <span class="text-body-sm font-medium text-[var(--text)]">All issues</span>
    <span class="text-micro text-[var(--text-faint)] tabular-nums">{total}</span>
    <div class="ml-auto flex items-center gap-1">
      <div class="relative">
        <button
          class="h-7 flex items-center gap-1 px-2 rounded-md text-caption font-medium
                 text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--bg-subtle)] transition-colors
                 {sortOpen ? 'text-[var(--text)] bg-[var(--bg-subtle)]' : ''}"
          title="Sort: {SORT_LABELS[sortField]} {sortDir === 'asc' ? 'ascending' : 'descending'}"
          onclick={(e) => { e.stopPropagation(); sortOpen = !sortOpen; }}
        >
          {#if sortDir === "asc"}<ArrowUp size={12} class="shrink-0" />{:else}<ArrowDown size={12} class="shrink-0" />{/if}
          <span>{SORT_LABELS[sortField]}</span>
        </button>
        {#if sortOpen}
          <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
          <div
            class="absolute right-0 top-full mt-1.5 z-30 w-[220px] bg-[var(--surface)] border border-[var(--border)]
                   rounded-lg shadow-lg py-1.5 text-body-sm"
            onclick={(e) => e.stopPropagation()}
          >
            <div class="px-3 pt-1 pb-1.5 text-[var(--text-faint)] text-micro uppercase tracking-widest font-semibold">
              Sort by
            </div>
            {#each [["priority", "Priority", Signal], ["age", "Age", Clock], ["updated", "Updated", History], ["number", "Issue number", Hash]] as const as [field, label, Icon] (field)}
              {@const active = sortField === field}
              <button
                class="w-full flex items-center justify-between gap-2 px-3 py-1.5 text-left transition-colors
                       {active
                  ? 'text-[var(--text)] bg-[var(--bg-subtle)] font-medium'
                  : 'text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--bg-subtle)]'}"
                onclick={() => selectSort(field)}
              >
                <span class="flex items-center gap-2"><Icon size={13} class="shrink-0" />{label}</span>
                {#if active}
                  <span class="text-[var(--accent)] flex items-center">
                    {#if sortDir === "asc"}<ArrowUp size={13} />{:else}<ArrowDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            {/each}
            <div class="px-3 pt-2 pb-1 mt-1 text-micro text-[var(--text-faint)] border-t border-[var(--border)] leading-snug">
              Click the active row to flip direction.
            </div>
          </div>
        {/if}
      </div>
      {#each [["/issues", "list", "List", List], ["/board", "board", "Board", LayoutGrid]] as const as [path, id, label, Icon] (id)}
        <button
          class="flex items-center gap-1.5 px-2 py-1 rounded-md text-body-sm transition-colors
                 {layout === id
                   ? 'bg-[var(--bg-subtle)] text-[var(--text)]'
                   : 'text-[var(--text-muted)] hover:text-[var(--text)]'}"
          aria-pressed={layout === id}
          onclick={() => navigate(path)}
        >
          <Icon size={14} />
          {label}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

{#if error}
  <ErrorState title="Couldn't load issues" message={error}>
    <button
      class="text-body-sm font-medium text-[var(--btn-success-text)] bg-[var(--btn-success)]
             px-3 py-1.5 rounded-md hover:bg-[var(--btn-success-hover)] transition-colors"
      onclick={() => { loading = true; error = ""; load(); }}
    >
      Try again
    </button>
  </ErrorState>
{:else if loading}
  <div class="h-full flex items-center justify-center">
    <div class="size-6 rounded-full border-2 border-[var(--border)] border-t-[var(--accent)] animate-spin"></div>
  </div>
{:else if layout === "board"}
  <div class="h-full flex overflow-x-auto snap-x snap-mandatory md:snap-none">
    {#each STATUSES as status (status)}
      {@const items = columns[status] ?? []}
      <div class="shrink-0 snap-start flex flex-col h-full w-[85vw] md:w-[300px]
                  border-r border-[var(--border)] last:border-r-0">
        <div class="flex items-center gap-2 px-3 py-2.5">
          <StatusIcon {status} size={14} />
          <span class="text-body-sm font-medium text-[var(--text)] capitalize">{statusLabel(status)}</span>
          <span class="text-micro text-[var(--text-faint)] tabular-nums">{items.length}</span>
        </div>
        <div
          class="flex flex-col gap-2 flex-1 min-h-[40px] overflow-y-auto px-2 pb-3"
          use:dndzone={{
            items,
            flipDurationMs: flipMs(),
            type: "lific-all-issues",
            dropTargetStyle: {
              outline: "2px dashed var(--accent)",
              outlineOffset: "-4px",
              borderRadius: "8px",
            },
          }}
          onconsider={(e) => consider(status, e)}
          onfinalize={(e) => finalize(status, e)}
        >
          {#each items as issue (issue.id)}
            <div animate:flip={{ duration: flipMs() }}>
              <IssueCard {issue} labels={[]} onOpen={open} onPeek={open} />
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{:else}
  <div class="h-full overflow-y-auto">
    {#each STATUSES as status (status)}
      {@const items = columns[status] ?? []}
      {#if items.length > 0}
        <details open={!TERMINAL_STATUSES.includes(status)}>
          <summary
            class="sticky top-0 z-10 flex items-center gap-2 px-6 py-2 cursor-pointer select-none
                   bg-[var(--bg-subtle)] border-b border-[var(--border)]"
          >
            <StatusIcon {status} size={14} />
            <span class="text-body-sm font-medium text-[var(--text)] capitalize">{statusLabel(status)}</span>
            <span class="text-micro text-[var(--text-faint)] tabular-nums">{items.length}</span>
          </summary>
          {#each items as issue (issue.id)}
            <button
              class="w-full flex items-center gap-2.5 px-6 py-2 text-left
                     border-b border-[var(--border)] hover:bg-[var(--bg-subtle)] transition-colors"
              onclick={() => open(issue)}
            >
              <PriorityIcon priority={issue.priority} size={15} />
              <span class="text-caption font-mono text-[var(--text-faint)] w-[72px] shrink-0 truncate">
                {issue.identifier}
              </span>
              <span class="text-body-sm text-[var(--text)] truncate flex-1">{issue.title}</span>
              <AssigneeMarker {issue} />
            </button>
          {/each}
        </details>
      {/if}
    {/each}
    {#if total === 0}
      <p class="py-16 text-center text-body-sm text-[var(--text-muted)]">No issues in any project yet.</p>
    {/if}
  </div>
{/if}
