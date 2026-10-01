<script lang="ts">
  // "An agent is working this right now": shown while the issue's newest
  // run-log line is younger than two minutes.
  import type { Issue } from "../api";
  import Tooltip from "../Tooltip.svelte";
  import { now } from "../now.svelte";
  import { isRunning } from "./runLog";
  import { runLogLiveAt } from "./runLogLive.svelte";

  let { issue }: { issue: Pick<Issue, "id" | "last_log_at"> } = $props();

  const running = $derived(isRunning(issue.last_log_at, runLogLiveAt(issue.id), now()));
</script>

{#if running}
  <Tooltip content="Running: the run log moved in the last two minutes">
    <span
      class="relative inline-flex size-2 shrink-0"
      data-running="true"
      role="img"
      aria-label="Running"
    >
      <span class="absolute inset-0 rounded-full bg-[var(--accent)] opacity-60 animate-ping"></span>
      <span class="relative size-2 rounded-full bg-[var(--accent)]"></span>
    </span>
  </Tooltip>
{/if}
