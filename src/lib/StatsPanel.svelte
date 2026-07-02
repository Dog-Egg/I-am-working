<script lang="ts">
  import { commands, type Stats, type HourlyWorkRecord } from "$lib/bindings";
  import { onMount } from "svelte";

  type Period = "day" | "week" | "month" | "year";

  type ChartBar = {
    label: string;
    seconds: number;
    showLabel: boolean;
    start: Date;
  };

  const periodOptions: Array<{ value: Period; label: string }> = [
    { value: "day", label: "日" },
    { value: "week", label: "周" },
    { value: "month", label: "月" },
    { value: "year", label: "年" },
  ];

  let { stats }: { stats: Stats } = $props();

  let period = $state<Period>("day");
  let periodOffset = $state(0);
  let records = $state<HourlyWorkRecord[]>([]);
  let isLoading = $state(false);
  let loadRequestId = 0;

  let chartData = $derived(buildChartData(period, records, periodOffset));
  let periodRangeLabel = $derived(formatPeriodRange(period, periodOffset));
  let isCurrentPeriod = $derived(periodOffset === 0);
  let canMoveNextPeriod = $derived(periodOffset < 0);
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

  function formatDurationMinutes(total: number): string {
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(h)}:${pad(m)}`;
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

  function getPeriodRange(selected: Period, offset = 0, base = new Date()) {
    if (selected === "day") {
      const start = startOfDay(addDays(base, offset));
      return { start, end: addDays(start, 1) };
    }

    if (selected === "week") {
      const start = startOfWeek(addDays(base, offset * 7));
      return { start, end: addDays(start, 7) };
    }

    if (selected === "month") {
      const start = new Date(base.getFullYear(), base.getMonth() + offset, 1);
      const end = new Date(start.getFullYear(), start.getMonth() + 1, 1);
      return { start, end };
    }

    const start = new Date(base.getFullYear() + offset, 0, 1);
    const end = new Date(start.getFullYear() + 1, 0, 1);
    return { start, end };
  }

  function formatDate(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${date.getFullYear()}年${month}月${day}日`;
  }

  function formatMonth(date: Date): string {
    const month = String(date.getMonth() + 1).padStart(2, "0");
    return `${date.getFullYear()}年${month}月`;
  }

  function formatCompactWeekRange(start: Date, end: Date): string {
    const endDay = addDays(end, -1);
    const startText = `${start.getFullYear()}年${start.getMonth() + 1}月${start.getDate()}日`;

    if (start.getFullYear() !== endDay.getFullYear()) {
      return `${startText} - ${endDay.getFullYear()}年${endDay.getMonth() + 1}月${endDay.getDate()}日`;
    }

    if (start.getMonth() !== endDay.getMonth()) {
      return `${startText} - ${endDay.getMonth() + 1}月${endDay.getDate()}日`;
    }

    return `${startText} - ${endDay.getDate()}日`;
  }

  function formatPeriodRange(selected: Period, offset: number): string {
    const { start, end } = getPeriodRange(selected, offset);

    if (selected === "day") return formatDate(start);
    if (selected === "month") return formatMonth(start);
    if (selected === "year") return `${start.getFullYear()}年`;

    return formatCompactWeekRange(start, end);
  }

  function selectPeriod(nextPeriod: Period) {
    period = nextPeriod;
    periodOffset = 0;
  }

  function selectPeriodRange(nextPeriod: Period, start: Date) {
    const today = startOfDay(new Date());
    const targetStart = startOfDay(start);

    if (targetStart > today) return;

    period = nextPeriod;

    if (nextPeriod === "day") {
      periodOffset = Math.floor(
        (targetStart.getTime() - today.getTime()) / 86400000,
      );
    } else {
      periodOffset =
        (targetStart.getFullYear() - today.getFullYear()) * 12 +
        targetStart.getMonth() -
        today.getMonth();
    }
  }

  function movePeriod(delta: number) {
    periodOffset = Math.min(0, periodOffset + delta);
  }

  function resetPeriod() {
    periodOffset = 0;
  }

  function toUnixSeconds(date: Date): number {
    return Math.floor(date.getTime() / 1000);
  }

  async function loadRecords(selected: Period = period, offset = periodOffset) {
    const requestId = ++loadRequestId;
    const { start, end } = getPeriodRange(selected, offset);
    isLoading = true;

    try {
      const result = await commands.getWorkRecords(
        toUnixSeconds(start),
        toUnixSeconds(end),
      );

      if (requestId === loadRequestId && result.status === "ok") {
        records = result.data;
      }
    } finally {
      if (requestId === loadRequestId) {
        isLoading = false;
      }
    }
  }

  function buildChartData(
    selected: Period,
    source: HourlyWorkRecord[],
    offset: number,
  ): ChartBar[] {
    const { start, end } = getPeriodRange(selected, offset);

    if (selected === "day") {
      const bars = Array.from({ length: 24 }, (_, hour) => ({
        label: String(hour).padStart(2, "0"),
        seconds: 0,
        showLabel: hour % 3 === 0,
        start: new Date(
          start.getFullYear(),
          start.getMonth(),
          start.getDate(),
          hour,
        ),
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
      const bars = labels.map((label, index) => ({
        label,
        seconds: 0,
        showLabel: true,
        start: addDays(start, index),
      }));

      for (const record of source) {
        const date = new Date(record.hour_start_unix * 1000);
        if (date >= start && date < end) {
          const offset = Math.floor(
            (startOfDay(date).getTime() - start.getTime()) / 86400000,
          );
          if (offset >= 0 && offset < bars.length) {
            bars[offset].seconds += record.work_seconds;
          }
        }
      }

      return bars;
    }

    if (selected === "month") {
      const dayCount = new Date(
        start.getFullYear(),
        start.getMonth() + 1,
        0,
      ).getDate();
      const bars = Array.from({ length: dayCount }, (_, index) => {
        const day = index + 1;
        return {
          label: String(day),
          seconds: 0,
          showLabel: day === 1 || day === dayCount || day % 5 === 0,
          start: new Date(start.getFullYear(), start.getMonth(), day),
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
      start: new Date(start.getFullYear(), month, 1),
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

  function getDrilldownPeriod(selected: Period): Period | null {
    if (selected === "week" || selected === "month") return "day";
    if (selected === "year") return "month";
    return null;
  }

  function canDrillDown(item: ChartBar): boolean {
    const nextPeriod = getDrilldownPeriod(period);
    return (
      nextPeriod !== null && startOfDay(item.start) <= startOfDay(new Date())
    );
  }

  function drillDown(item: ChartBar) {
    const nextPeriod = getDrilldownPeriod(period);
    if (!nextPeriod) return;

    selectPeriodRange(nextPeriod, item.start);
  }

  $effect(() => {
    const selected = period;
    const offset = periodOffset;
    void loadRecords(selected, offset);
  });

  onMount(() => {
    const refreshTimer = window.setInterval(() => {
      void loadRecords(period, periodOffset);
    }, 60000);

    return () => {
      window.clearInterval(refreshTimer);
    };
  });
</script>

<section class="flex flex-wrap items-end justify-between gap-4">
  <div class="flex flex-col gap-1">
    <span class="text-xs font-medium uppercase tracking-widest text-zinc-500"
      >今日工作时长</span
    >
    <span class="font-mono text-5xl font-semibold tabular-nums">
      {formatDuration(stats.today_work_seconds)}
    </span>
  </div>
</section>

<section
  class="rounded-lg border border-zinc-800 bg-zinc-900/80 p-5 shadow-2xl shadow-black/20"
>
  <div class="mb-5 flex flex-wrap items-start justify-between gap-3">
    <div class="flex items-baseline gap-3">
      <h1 class="text-lg font-semibold text-zinc-100">统计</h1>
      <span class="font-mono text-sm tabular-nums text-zinc-400">
        {formatDuration(chartTotalSeconds)}
      </span>
    </div>

    <div class="flex w-[22rem] max-w-full flex-col items-center gap-2">
      <div
        class="grid w-full grid-cols-4 rounded-lg border border-zinc-700 bg-zinc-950 p-1"
      >
        {#each periodOptions as option}
          <button
            class="rounded-md px-4 py-1.5 text-sm font-medium transition
            {period === option.value
              ? 'bg-cyan-500 text-zinc-950'
              : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
            type="button"
            onclick={() => selectPeriod(option.value)}
          >
            {option.label}
          </button>
        {/each}
      </div>

      <div class="flex w-full items-center gap-1">
        <button
          aria-label="上一周期"
          class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-zinc-300 transition hover:bg-zinc-800 hover:text-zinc-100"
          type="button"
          onclick={() => movePeriod(-1)}
        >
          <svg
            aria-hidden="true"
            class="h-4 w-4"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="m15 18-6-6 6-6" />
          </svg>
        </button>
        <div class="min-w-0 flex-1 flex items-center justify-center gap-2">
          <span class="text-center font-mono text-sm tabular-nums text-zinc-400">
            {periodRangeLabel}
          </span>
          <button
            aria-label="回到当前周期"
            class={[
              isCurrentPeriod && "hidden",
              "flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-zinc-300 transition hover:bg-zinc-800 hover:text-zinc-100",
            ]}
            type="button"
            disabled={isCurrentPeriod}
            onclick={resetPeriod}
          >
            <svg
              aria-hidden="true"
              class="size-3"
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              viewBox="0 0 24 24"
            >
              <path d="M3 12a9 9 0 1 0 3-6.7" />
              <path d="M3 3v6h6" />
            </svg>
          </button>
        </div>
        <button
          aria-label="下一周期"
          class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-zinc-300 transition hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-zinc-950 disabled:hover:text-zinc-300"
          type="button"
          disabled={!canMoveNextPeriod}
          onclick={() => movePeriod(1)}
        >
          <svg
            aria-hidden="true"
            class="h-4 w-4"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="m9 18 6-6-6-6" />
          </svg>
        </button>
      </div>
    </div>
  </div>

  <div class="relative flex h-64 items-end gap-1 border-b border-zinc-700 pb-8">
    {#if isLoading}
      <div class="absolute right-0 top-0 text-xs text-zinc-500">
        <!-- 同步中 -->
      </div>
    {/if}

    {#each chartData as item}
      <div class="flex min-w-0 flex-1 flex-col items-center justify-end gap-2">
        <button
          class="group relative flex w-full max-w-10 justify-center disabled:cursor-default"
          aria-label={`${item.label} ${formatDurationMinutes(item.seconds)}`}
          type="button"
          disabled={!canDrillDown(item)}
          onclick={() => drillDown(item)}
        >
          <div
            class="pointer-events-none absolute bottom-full mb-2 rounded bg-zinc-950 px-2 py-1 font-mono text-[10px] leading-none text-zinc-100 opacity-0 shadow-lg shadow-black/30 transition-opacity group-hover:opacity-100"
          >
            {formatDurationMinutes(item.seconds)}
          </div>
          <div
            class="w-full rounded-t bg-cyan-400 transition-all group-enabled:group-hover:bg-cyan-300"
            style:height={barHeight(item.seconds)}
          ></div>
        </button>
        <span class="h-4 text-[10px] leading-4 text-zinc-500">
          {item.showLabel ? item.label : ""}
        </span>
      </div>
    {/each}
  </div>
</section>
