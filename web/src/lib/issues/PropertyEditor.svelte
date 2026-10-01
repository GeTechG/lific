<script lang="ts">
  // The issue sidebar's "Properties" field: free-form name → value text pairs
  // (a path footprint, a PR number, tool state). Editors add, change and
  // remove them; each edit is one server-side delta, so two people editing
  // different properties never overwrite each other.
  import { updateIssue, type Issue } from "../api";
  import { Plus, X } from "lucide-svelte";
  import { toast } from "../toast/toast.svelte";

  let {
    issueId,
    properties,
    editable,
    onChange,
  }: {
    issueId: number;
    properties: Record<string, string>;
    editable: boolean;
    /** The issue as the server returned it after a successful edit. */
    onChange: (issue: Issue) => void;
  } = $props();

  // Mirrors the server's rule; the server is still the one that decides.
  const NAME_PATTERN = "[a-z0-9][a-z0-9_\\-]{0,63}";
  const MAX_VALUE_LENGTH = 4096;

  const entries = $derived(Object.entries(properties).sort(([a], [b]) => a.localeCompare(b)));

  /** "" when no form is open, "+" for the add form, else the name being edited. */
  let editing = $state("");
  let name = $state("");
  let value = $state("");
  let busy = $state(false);

  function open(target: string) {
    editing = target;
    name = target === "+" ? "" : target;
    value = target === "+" ? "" : (properties[target] ?? "");
  }

  async function save(input: Parameters<typeof updateIssue>[1], failure: string) {
    if (busy) return;
    busy = true;
    const res = await updateIssue(issueId, input);
    busy = false;
    if (!res.ok) {
      toast(`${failure}: ${res.error}`, { kind: "error" });
      return;
    }
    editing = "";
    onChange(res.data);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!value.trim()) return;
    void save({ set_properties: { [name]: value } }, `Couldn't set ${name}`);
  }

  function remove(target: string) {
    void save({ unset_properties: [target] }, `Couldn't remove ${target}`);
  }
</script>

<div class="issue-meta-field" data-testid="issue-properties">
  <p class="issue-meta-field-label">Properties</p>
  {#if entries.length > 0}
    <ul class="flex flex-col gap-1.5 m-0 p-0 list-none">
      {#each entries as [key, text] (key)}
        {#if editing !== key}
          <li class="flex items-start gap-2" data-property={key}>
            <div class="flex-1 min-w-0">
              <p class="m-0 font-mono text-caption text-[var(--text-muted)] leading-snug break-all">
                {key}
              </p>
              {#if editable}
                <button
                  type="button"
                  class="block w-full text-left text-body-sm text-[var(--text)] leading-snug
                         break-words whitespace-pre-wrap rounded px-1 -mx-1
                         hover:bg-[var(--bg-subtle)] transition-colors"
                  title="Change value"
                  onclick={() => open(key)}
                >
                  {text}
                </button>
              {:else}
                <p class="m-0 text-body-sm text-[var(--text)] leading-snug break-words whitespace-pre-wrap">
                  {text}
                </p>
              {/if}
            </div>
            {#if editable}
              <button
                type="button"
                class="size-6 -my-0.5 shrink-0 grid place-items-center rounded
                       text-[var(--text-faint)] hover:text-[var(--error)]
                       hover:bg-[var(--bg-subtle)] transition-colors disabled:opacity-40"
                title="Remove this property"
                aria-label="Remove property {key}"
                disabled={busy}
                onclick={() => remove(key)}
              >
                <X size={13} />
              </button>
            {/if}
          </li>
        {/if}
      {/each}
    </ul>
  {/if}

  {#if editable}
    {#if editing}
      <form
        class="flex flex-col gap-2 rounded-md border border-[var(--border)] p-2.5 -mx-1"
        onsubmit={submit}
        aria-label={editing === "+" ? "Add property" : `Change property ${editing}`}
      >
        <label class="flex flex-col gap-1 text-caption text-[var(--text-muted)]">
          Name
          <input
            type="text"
            class="font-mono text-body-sm rounded-md border border-[var(--border)] bg-[var(--bg-subtle)]
                   text-[var(--text)] px-2 py-1.5 outline-none focus:border-[var(--accent)]
                   disabled:opacity-60"
            placeholder="footprint"
            pattern={NAME_PATTERN}
            title="Lowercase letters, digits, '_' and '-'; up to 64 characters"
            maxlength="64"
            autocapitalize="none"
            spellcheck="false"
            required
            disabled={editing !== "+"}
            bind:value={name}
          />
        </label>
        <label class="flex flex-col gap-1 text-caption text-[var(--text-muted)]">
          Value
          <textarea
            class="text-body-sm rounded-md border border-[var(--border)] bg-[var(--bg-subtle)]
                   text-[var(--text)] px-2 py-1.5 outline-none focus:border-[var(--accent)] resize-y"
            rows="2"
            maxlength={MAX_VALUE_LENGTH}
            required
            bind:value
          ></textarea>
        </label>
        <div class="flex gap-2 justify-end">
          <button
            type="button"
            class="text-body-sm text-[var(--text-muted)] px-3 py-1.5 rounded-md hover:bg-[var(--bg-subtle)] transition-colors"
            onclick={() => (editing = "")}
          >
            Cancel
          </button>
          <button
            type="submit"
            class="text-body-sm font-medium text-[var(--btn-success-text)] bg-[var(--btn-success)]
                   px-3 py-1.5 rounded-md hover:bg-[var(--btn-success-hover)] transition-colors
                   disabled:opacity-40 disabled:cursor-not-allowed"
            disabled={busy || !value.trim()}
          >
            {editing === "+" ? "Add" : "Save"}
          </button>
        </div>
      </form>
    {:else}
      <button
        type="button"
        class="self-start flex items-center gap-1.5 text-body-sm text-[var(--text-muted)]
               rounded-md px-2 py-1 -mx-2 hover:bg-[var(--bg-subtle)] hover:text-[var(--text)] transition-colors"
        onclick={() => open("+")}
      >
        <Plus size={13} />
        Add property
      </button>
    {/if}
  {/if}
</div>
