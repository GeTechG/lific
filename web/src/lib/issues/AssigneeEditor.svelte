<script lang="ts">
  // The issue sidebar's "Assignee" field: one optional account, picked from
  // the project's members, its administrators and their agents.
  import { listAssignees, updateIssue, type AssigneeCandidate, type Issue } from "../api";
  import Select from "../Select.svelte";
  import AssigneeChip from "./AssigneeChip.svelte";
  import { toast } from "../toast/toast.svelte";

  let {
    issue,
    editable,
    onChange,
  }: {
    issue: Issue;
    editable: boolean;
    /** The issue as the server returned it after a successful edit. */
    onChange: (issue: Issue) => void;
  } = $props();

  let candidates = $state<AssigneeCandidate[]>([]);
  $effect(() => {
    if (!editable) return;
    void listAssignees(issue.project_id).then((res) => {
      if (res.ok) candidates = res.data;
    });
  });

  const label = (c: AssigneeCandidate) =>
    `${c.display_name || c.username}${c.is_bot ? " (agent)" : ""}`;

  // The current assignee stays selectable even after leaving the project.
  const options = $derived([
    { value: null, label: "Unassigned" },
    ...candidates.map((c) => ({ value: c.username, label: label(c) })),
    ...(issue.assignee && !candidates.some((c) => c.username === issue.assignee)
      ? [{ value: issue.assignee, label: issue.assignee_display_name || issue.assignee }]
      : []),
  ]);

  // What the picker shows: the issue's assignee, until a pick is refused.
  let picked = $state<string | number | null>(null);
  $effect(() => {
    picked = issue.assignee ?? null;
  });

  async function assign(username: string | null) {
    if (username === (issue.assignee ?? null)) return;
    const res = await updateIssue(issue.id, { assignee: username });
    if (!res.ok) {
      picked = issue.assignee ?? null;
      toast(`Couldn't change the assignee: ${res.error}`, { kind: "error" });
      return;
    }
    onChange(res.data);
  }
</script>

<div class="issue-meta-field" data-testid="issue-assignee">
  <p class="issue-meta-field-label">Assignee</p>
  <div class="flex items-center gap-2 min-w-0">
    <AssigneeChip {issue} />
    {#if editable}
      <Select
        {options}
        bind:value={picked}
        size="sm"
        class="flex-1 min-w-0"
        onchange={(opt) => assign(opt.value as string | null)}
      />
    {:else}
      <span class="text-body-sm text-[var(--text)] truncate">
        {issue.assignee_display_name || issue.assignee}
      </span>
    {/if}
  </div>
</div>
