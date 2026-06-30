<script lang="ts">
  import { events, type LogMessage } from "$lib/bindings";
  import { onMount } from "svelte";

  const MAX_LOGS = 1000;
  let logs = $state<LogMessage[]>([]);
  let unlisten: (() => void) | null = null;
  let logContainer: HTMLDivElement | null = null;
  let autoScroll = true;

  onMount(() => {
    let disposed = false;
    void events.logMessage
      .listen((event) => {
        if (disposed) return;
        const next =
          logs.length >= MAX_LOGS
            ? [...logs.slice(-(MAX_LOGS - 1)), event.payload]
            : [...logs, event.payload];
        logs = next;
      })
      .then((nextUnlisten) => {
        if (disposed) {
          nextUnlisten();
        } else {
          unlisten = nextUnlisten;
        }
      });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  });

  $effect(() => {
    // 依赖 logs.length，新日志到达后触发滚动
    void logs.length;
    if (autoScroll && logContainer) {
      logContainer.scrollTop = logContainer.scrollHeight;
    }
  });

  function onScroll() {
    if (!logContainer) return;
    const atBottom =
      logContainer.scrollHeight -
        logContainer.scrollTop -
        logContainer.clientHeight <
      20;
    autoScroll = atBottom;
  }

  function clearLogs() {
    logs = [];
  }
</script>

<main class="min-h-screen w-screen bg-zinc-950 text-zinc-100">
  <div class="mx-auto flex h-screen max-w-5xl flex-col gap-3 px-4 py-4">
    <header
      class="flex items-center justify-between border-b border-zinc-800 pb-3"
    >
      <h1 class="text-lg font-semibold">日志</h1>
      <div class="flex items-center gap-3">
        <span class="text-xs text-zinc-500">{logs.length} / {MAX_LOGS}</span>
        <button
          class="rounded-md border border-zinc-700 px-3 py-1 text-xs text-zinc-300 transition hover:bg-zinc-800"
          type="button"
          onclick={clearLogs}
        >
          清空
        </button>
      </div>
    </header>
    <div
      bind:this={logContainer}
      onscroll={onScroll}
      class="flex-1 overflow-auto font-mono text-xs leading-5"
    >
      {#each logs as log, i (i)}
        <div class="flex gap-2 px-1 py-0.5 hover:bg-zinc-900">
          <span class="shrink-0 text-zinc-500">{log.timestamp}</span>
          <span class="break-all text-zinc-200">{log.message}</span>
        </div>
      {:else}
        <div class="px-1 py-2 text-zinc-600">暂无日志</div>
      {/each}
    </div>
  </div>
</main>
