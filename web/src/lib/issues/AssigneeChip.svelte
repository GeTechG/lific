<script lang="ts">
  // The compact "who is working this" indicator on list rows, board cards
  // and the sidebar: initials for a person, a bot glyph for an agent.
  import type { Issue } from "../api";
  import { Bot } from "lucide-svelte";
  import Tooltip from "../Tooltip.svelte";

  let { issue }: { issue: Pick<Issue, "assignee" | "assignee_display_name" | "assignee_is_bot"> } =
    $props();

  const name = $derived(issue.assignee_display_name || issue.assignee || "");
  const initials = $derived(
    name
      .split(/[\s._-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part[0].toUpperCase())
      .join(""),
  );
</script>

{#if issue.assignee}
  <Tooltip content={`Assigned to ${name}${issue.assignee_is_bot ? " (agent)" : ""}`}>
    <span
      class="inline-grid place-items-center size-5 shrink-0 rounded-full text-micro font-semibold
             leading-none select-none
             {issue.assignee_is_bot
        ? 'bg-[var(--accent-subtle)] text-[var(--accent)]'
        : 'bg-[var(--bg-subtle)] text-[var(--text-muted)] border border-[var(--border)]'}"
      data-assignee={issue.assignee}
      data-assignee-bot={issue.assignee_is_bot ? "true" : undefined}
      aria-label={`Assigned to ${name}`}
    >
      {#if issue.assignee_is_bot}
        <Bot size={12} />
      {:else}
        {initials}
      {/if}
    </span>
  </Tooltip>
{/if}
