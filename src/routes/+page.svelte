<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Stats = {
    work_seconds: number;
    is_active: boolean;
    idle_seconds: number;
  };

  let stats = $state<Stats>({
    work_seconds: 0,
    is_active: false,
    idle_seconds: 0,
  });
  let unlisten: (() => void) | null = null;

  function formatDuration(total: number): string {
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(h)}:${pad(m)}:${pad(s)}`;
  }

  onMount(async () => {
    stats = await invoke<Stats>("get_stats");
    unlisten = await listen<Stats>("stats-updated", (e) => {
      stats = e.payload;
    });
  });

  $effect(() => {
    return () => {
      if (unlisten) unlisten();
    };
  });
</script>

<main
  class="select-none flex flex-col items-center justify-center gap-5 w-screen h-screen bg-slate-900 text-slate-100"
>
  <div class="flex flex-col items-center gap-1">
    <span class="text-xs uppercase tracking-widest text-slate-400"
      >工作时间</span
    >
    <span class="font-mono text-4xl font-semibold tabular-nums">
      {formatDuration(stats.work_seconds)}
    </span>
  </div>

  <div
    class="flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium
    {stats.is_active
      ? 'bg-emerald-500/15 text-emerald-400'
      : 'bg-slate-500/15 text-slate-400'}"
  >
    <span
      class="w-2 h-2 rounded-full {stats.is_active
        ? 'bg-emerald-400 animate-pulse'
        : 'bg-slate-500'}"
    ></span>
    {stats.is_active ? "工作中" : "空闲"}
    {#if !stats.is_active}
      <span class="text-xs text-slate-500">· 已空闲 {stats.idle_seconds}s</span>
    {/if}
  </div>
</main>
