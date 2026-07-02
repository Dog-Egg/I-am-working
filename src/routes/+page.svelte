<script lang="ts">
  import { commands, events, type Stats } from "$lib/bindings";
  import SettingsPanel from "$lib/SettingsPanel.svelte";
  import StatsPanel from "$lib/StatsPanel.svelte";
  import { onMount } from "svelte";

  type AppTab = "stats" | "settings";

  const tabOptions: Array<{ value: AppTab; label: string }> = [
    { value: "stats", label: "统计" },
    { value: "settings", label: "设置" },
  ];

  let stats = $state<Stats>({
    today_work_seconds: 0,
    is_active: false,
    idle_seconds: 0,
  });
  let activeTab = $state<AppTab>("stats");

  onMount(() => {
    let unlistenStats: (() => void) | null = null;
    let unlistenTab: (() => void) | null = null;
    let disposed = false;

    void commands.getStats().then((nextStats) => {
      if (!disposed) stats = nextStats;
    });

    void events.statsUpdated
      .listen((event) => {
        stats = event.payload;
      })
      .then((nextUnlisten) => {
        if (disposed) {
          nextUnlisten();
        } else {
          unlistenStats = nextUnlisten;
        }
      });

    void events.showTab
      .listen((event) => {
        activeTab = event.payload as AppTab;
      })
      .then((nextUnlisten) => {
        if (disposed) {
          nextUnlisten();
        } else {
          unlistenTab = nextUnlisten;
        }
      });

    return () => {
      disposed = true;
      if (unlistenStats) unlistenStats();
      if (unlistenTab) unlistenTab();
    };
  });
</script>

<main class="min-h-screen w-screen bg-zinc-950 text-zinc-100">
  <div class="mx-auto flex min-h-screen max-w-5xl flex-col gap-5 px-6 py-6">
    <header class="flex flex-wrap items-center justify-between gap-4">
      <nav
        class="grid grid-cols-2 rounded-lg border border-zinc-700 bg-zinc-950 p-1"
      >
        {#each tabOptions as option}
          <button
            class="rounded-md px-5 py-1.5 text-sm font-medium transition
            {activeTab === option.value
              ? 'bg-cyan-500 text-zinc-950'
              : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
            type="button"
            onclick={() => (activeTab = option.value)}
          >
            {option.label}
          </button>
        {/each}
      </nav>

      <div
        class="flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium
        {stats.is_active
          ? 'bg-emerald-500/15 text-emerald-300'
          : 'bg-zinc-700/60 text-zinc-400'}"
      >
        <span
          class="h-2 w-2 rounded-full {stats.is_active
            ? 'animate-pulse bg-emerald-300'
            : 'bg-zinc-500'}"
        ></span>
        {stats.is_active ? "工作中" : "空闲"}
        {#if !stats.is_active}
          <span class="text-xs text-zinc-500">· {stats.idle_seconds}s</span>
        {/if}
      </div>
    </header>

    {#if activeTab === "stats"}
      <StatsPanel {stats} />
    {:else}
      <SettingsPanel />
    {/if}
  </div>
</main>
