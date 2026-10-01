<script lang="ts">
  // The issue page's "Run log": what the agent working this issue is doing,
  // as the lines its scheduler appends. Read-only, chronological, and live:
  // new lines arrive without a reload. Hidden while the issue has none.
  import { tick } from "svelte";
  import { listIssueLog, type Issue, type IssueLogLine } from "../api";
  import { REALTIME_INVALIDATE_EVENT, type RealtimeEvent } from "../autoRefresh.svelte";
  import { runLogAppended, logTime } from "./runLog";
  import RunningDot from "./RunningDot.svelte";

  let { issue }: { issue: Pick<Issue, "id" | "last_log_at"> } = $props();

  const PAGE = 200;

  let lines = $state<IssueLogLine[]>([]);
  let hasOlder = $state(false);
  let loadingOlder = $state(false);
  let box = $state<HTMLDivElement | null>(null);
  let loadedFor = 0;
  let pulling = false;
  let pullAgain = false;

  function atBottom(): boolean {
    return !box || box.scrollHeight - box.scrollTop - box.clientHeight < 24;
  }

  async function scrollToBottom() {
    await tick();
    if (box) box.scrollTop = box.scrollHeight;
  }

  /** Fetch whatever was appended after the last line we hold. */
  async function pull() {
    if (pulling) {
      pullAgain = true;
      return;
    }
    pulling = true;
    const issueId = issue.id;
    try {
      do {
        pullAgain = false;
        const first = lines.length === 0;
        const res = await listIssueLog(
          issueId,
          first ? { limit: PAGE } : { after: lines[lines.length - 1].id, limit: PAGE },
        );
        if (!res.ok || issueId !== issue.id) return;
        if (res.data.length === 0) continue;
        // Follow the tail only for a reader who is already at it.
        const follow = first || atBottom();
        if (first) hasOlder = res.data.length === PAGE;
        else if (res.data.length === PAGE) pullAgain = true;
        lines = first ? res.data : [...lines, ...res.data];
        if (follow) await scrollToBottom();
      } while (pullAgain);
    } finally {
      pulling = false;
    }
  }

  async function loadOlder() {
    if (loadingOlder || lines.length === 0) return;
    loadingOlder = true;
    const res = await listIssueLog(issue.id, { before: lines[0].id, limit: PAGE });
    loadingOlder = false;
    if (!res.ok) return;
    hasOlder = res.data.length === PAGE;
    // Keep the line the reader was looking at where it is.
    const height = box?.scrollHeight ?? 0;
    lines = [...res.data, ...lines];
    await tick();
    if (box) box.scrollTop += box.scrollHeight - height;
  }

  // (Re)load when the issue changes, and catch up whenever a refetch of the
  // issue reports a newer line: the path that still works without a socket.
  $effect(() => {
    if (issue.id !== loadedFor) {
      loadedFor = issue.id;
      lines = [];
      hasOlder = false;
    }
    if (issue.last_log_at) void pull();
  });

  $effect(() => {
    const onRealtime = (event: Event) => {
      const log = runLogAppended((event as CustomEvent<RealtimeEvent>).detail);
      if (log?.issue_id === issue.id) void pull();
    };
    window.addEventListener(REALTIME_INVALIDATE_EVENT, onRealtime);
    return () => window.removeEventListener(REALTIME_INVALIDATE_EVENT, onRealtime);
  });

  function clock(ts: string): string {
    return new Date(logTime(ts)).toLocaleTimeString([], { hour12: false });
  }
</script>

{#if lines.length > 0}
  <section class="mt-6" data-testid="issue-run-log" aria-label="Run log">
    <h2 class="flex items-center gap-2 text-caption font-semibold uppercase tracking-wider text-[var(--text-faint)] mb-2">
      Run log
      <RunningDot {issue} />
    </h2>
    <div
      bind:this={box}
      class="max-h-[360px] overflow-y-auto rounded-md border border-[var(--border)]
             bg-[var(--bg-subtle)] px-3 py-2 font-mono text-caption leading-relaxed"
      role="log"
      aria-live="off"
    >
      {#if hasOlder}
        <button
          type="button"
          class="block mx-auto mb-1 font-sans text-caption text-[var(--text-muted)] hover:text-[var(--text)]
                 disabled:opacity-50"
          disabled={loadingOlder}
          onclick={loadOlder}
        >
          {loadingOlder ? "Loading…" : "Show older lines"}
        </button>
      {/if}
      {#each lines as line, i (line.id)}
        {#if line.source && (i === 0 || lines[i - 1].source !== line.source)}
          <div class="mt-1 first:mt-0 text-micro uppercase tracking-wider text-[var(--text-faint)]" data-log-source>
            {line.source}
          </div>
        {/if}
        <div class="flex gap-2" data-log-line={line.id}>
          <time class="shrink-0 text-[var(--text-faint)] tabular-nums" datetime={line.ts}>{clock(line.ts)}</time>
          <span class="min-w-0 whitespace-pre-wrap break-words text-[var(--text)]">{line.text}</span>
        </div>
      {/each}
    </div>
  </section>
{/if}
