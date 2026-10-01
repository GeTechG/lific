// When each issue's run log last grew, as seen on the realtime channel.
// A log line does not re-deliver the issue row (it changes no seq), so the
// "running" indicator on lists and boards learns about it here.
import { SvelteMap } from "svelte/reactivity";
import { REALTIME_INVALIDATE_EVENT, type RealtimeEvent } from "../autoRefresh.svelte";
import { runLogAppended } from "./runLog";

const liveAt = new SvelteMap<number, number>();
let listening = false;

function listen() {
  if (listening || typeof window === "undefined") return;
  listening = true;
  window.addEventListener(REALTIME_INVALIDATE_EVENT, (event) => {
    const log = runLogAppended((event as CustomEvent<RealtimeEvent>).detail);
    if (log) liveAt.set(log.issue_id, Date.now());
  });
}

/** Local epoch ms of the last line seen live for this issue, if any.
 *  Reactive: reading it subscribes to later lines. */
export function runLogLiveAt(issueId: number): number | undefined {
  listen();
  return liveAt.get(issueId);
}
