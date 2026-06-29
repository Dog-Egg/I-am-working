<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Stats = {
    today_work_seconds: number;
    is_active: boolean;
    idle_seconds: number;
  };

  type HourlyWorkRecord = {
    hour_start_unix: number;
    work_seconds: number;
  };

  type Period = "day" | "week" | "month" | "year";

  type ChartBar = {
    label: string;
    seconds: number;
    showLabel: boolean;
  };

  const periodOptions: Array<{ value: Period; label: string }> = [
    { value: "day", label: "日" },
    { value: "week", label: "周" },
    { value: "month", label: "月" },
    { value: "year", label: "年" },
  ];

  let stats = $state<Stats>({
    today_work_seconds: 0,
    is_active: false,
    idle_seconds: 0,
  });
  let period = $state<Period>("day");
  let records = $state<HourlyWorkRecord[]>([]);
  let isLoading = $state(false);
  let loadRequestId = 0;

  let chartData = $derived(buildChartData(period, records));
  let maxSeconds = $derived(
    Math.max(1, ...chartData.map((item) => item.seconds)),
  );
  let chartTotalSeconds = $derived(
    chartData.reduce((total, item) => total + item.seconds, 0),
  );

  function formatDuration(total: number): string {
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(h)}:${pad(m)}:${pad(s)}`;
  }

  function startOfDay(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate());
  }

  function addDays(date: Date, days: number): Date {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
  }

  function startOfWeek(date: Date): Date {
    const start = startOfDay(date);
    const day = start.getDay();
    start.setDate(start.getDate() + (day === 0 ? -6 : 1 - day));
    return start;
  }

  function getPeriodRange(selected: Period, base = new Date()) {
    if (selected === "day") {
      const start = startOfDay(base);
      return { start, end: addDays(start, 1) };
    }

    if (selected === "week") {
      const start = startOfWeek(base);
      return { start, end: addDays(start, 7) };
    }

    if (selected === "month") {
      const start = new Date(base.getFullYear(), base.getMonth(), 1);
      const end = new Date(base.getFullYear(), base.getMonth() + 1, 1);
      return { start, end };
    }

    const start = new Date(base.getFullYear(), 0, 1);
    const end = new Date(base.getFullYear() + 1, 0, 1);
    return { start, end };
  }

  function toUnixSeconds(date: Date): number {
    return Math.floor(date.getTime() / 1000);
  }

  async function loadRecords(selected: Period = period) {
    const requestId = ++loadRequestId;
    const { start, end } = getPeriodRange(selected);
    isLoading = true;

    try {
      const nextRecords = await invoke<HourlyWorkRecord[]>("get_work_records", {
        startUnix: toUnixSeconds(start),
        endUnix: toUnixSeconds(end),
      });

      if (requestId === loadRequestId) {
        records = nextRecords;
      }
    } finally {
      if (requestId === loadRequestId) {
        isLoading = false;
      }
    }
  }

  function buildChartData(selected: Period, source: HourlyWorkRecord[]): ChartBar[] {
    const { start, end } = getPeriodRange(selected);

    if (selected === "day") {
      const bars = Array.from({ length: 24 }, (_, hour) => ({
        label: String(hour).padStart(2, "0"),
        seconds: 0,
        showLabel: hour % 3 === 0,
      }));

      for (const record of source) {
        const date = new Date(record.hour_start_unix * 1000);
        if (date >= start && date < end) {
          bars[date.getHours()].seconds += record.work_seconds;
        }
      }

      return bars;
    }

    if (selected === "week") {
      const labels = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
      const bars = labels.map((label) => ({ label, seconds: 0, showLabel: true }));

      for (const record of source) {
        const date = new Date(record.hour_start_unix * 1000);
        if (date >= start && date < end) {
          const offset = Math.floor((startOfDay(date).getTime() - start.getTime()) / 86400000);
          if (offset >= 0 && offset < bars.length) {
            bars[offset].seconds += record.work_seconds;
          }
        }
      }

      return bars;
    }

    if (selected === "month") {
      const dayCount = new Date(start.getFullYear(), start.getMonth() + 1, 0).getDate();
      const bars = Array.from({ length: dayCount }, (_, index) => {
        const day = index + 1;
        return {
          label: String(day),
          seconds: 0,
          showLabel: day === 1 || day === dayCount || day % 5 === 0,
        };
      });

      for (const record of source) {
        const date = new Date(record.hour_start_unix * 1000);
        if (date >= start && date < end) {
          bars[date.getDate() - 1].seconds += record.work_seconds;
        }
      }

      return bars;
    }

    const bars = Array.from({ length: 12 }, (_, month) => ({
      label: `${month + 1}月`,
      seconds: 0,
      showLabel: true,
    }));

    for (const record of source) {
      const date = new Date(record.hour_start_unix * 1000);
      if (date >= start && date < end) {
        bars[date.getMonth()].seconds += record.work_seconds;
      }
    }

    return bars;
  }

  function barHeight(seconds: number): string {
    if (seconds === 0) return "2px";
    return `${Math.max(8, Math.round((seconds / maxSeconds) * 190))}px`;
  }

  $effect(() => {
    const selected = period;
    void loadRecords(selected);
  });

  onMount(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;

    void invoke<Stats>("get_stats").then((nextStats) => {
      if (!disposed) stats = nextStats;
    });

    void listen<Stats>("stats-updated", (event) => {
      stats = event.payload;
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
      } else {
        unlisten = nextUnlisten;
      }
    });

    const refreshTimer = window.setInterval(() => {
      void loadRecords(period);
    }, 60000);

    return () => {
      disposed = true;
      window.clearInterval(refreshTimer);
      if (unlisten) unlisten();
    };
  });
</script>

<main class="min-h-screen w-screen bg-zinc-950 text-zinc-100">
  <div class="mx-auto flex min-h-screen max-w-5xl flex-col gap-5 px-6 py-6">
    <section class="flex flex-wrap items-end justify-between gap-4">
      <div class="flex flex-col gap-1">
        <span class="text-xs font-medium uppercase tracking-widest text-zinc-500"
          >今日工作时长</span
        >
        <span class="font-mono text-5xl font-semibold tabular-nums">
          {formatDuration(stats.today_work_seconds)}
        </span>
      </div>

      <div class="flex items-center gap-3">
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
      </div>
    </section>

    <section class="rounded-lg border border-zinc-800 bg-zinc-900/80 p-5 shadow-2xl shadow-black/20">
      <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
        <div class="flex items-baseline gap-3">
          <h1 class="text-lg font-semibold text-zinc-100">统计</h1>
          <span class="font-mono text-sm tabular-nums text-zinc-400">
            {formatDuration(chartTotalSeconds)}
          </span>
        </div>

        <div class="grid grid-cols-4 rounded-lg border border-zinc-700 bg-zinc-950 p-1">
          {#each periodOptions as option}
            <button
              class="rounded-md px-4 py-1.5 text-sm font-medium transition
              {period === option.value
                ? 'bg-cyan-500 text-zinc-950'
                : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
              type="button"
              onclick={() => (period = option.value)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="relative flex h-64 items-end gap-1 border-b border-zinc-700 pb-8">
        {#if isLoading}
          <div class="absolute right-0 top-0 text-xs text-zinc-500">同步中</div>
        {/if}

        {#each chartData as item}
          <div class="flex min-w-0 flex-1 flex-col items-center justify-end gap-2">
            <div
              class="w-full max-w-10 rounded-t bg-cyan-400 transition-all"
              style:height={barHeight(item.seconds)}
              title={`${item.label} ${formatDuration(item.seconds)}`}
            ></div>
            <span class="h-4 text-[10px] leading-4 text-zinc-500">
              {item.showLabel ? item.label : ""}
            </span>
          </div>
        {/each}
      </div>
    </section>
  </div>
</main>
