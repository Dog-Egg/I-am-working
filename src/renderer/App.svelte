<script lang="ts">
  import { onMount } from "svelte";

  const MIN_SECONDS = 1;
  const MAX_SECONDS = 60 * 60;
  const DEFAULT_SECONDS = 25 * 60;

  type TimeUnit = "seconds" | "minutes" | "hours";

  let amount = 25;
  let unit: TimeUnit = "minutes";
  let todayWorkedSeconds = 0;
  let statusText = "请输入 1 秒到 1 小时之间的时间。";
  let isSaving = false;

  const unitToSeconds = (selectedUnit: TimeUnit): number => {
    if (selectedUnit === "hours") {
      return 60 * 60;
    }

    if (selectedUnit === "minutes") {
      return 60;
    }

    return 1;
  };

  const formatDuration = (totalSeconds: number): string => {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    const parts: string[] = [];

    if (hours > 0) {
      parts.push(`${hours} 小时`);
    }

    if (minutes > 0) {
      parts.push(`${minutes} 分钟`);
    }

    if (seconds > 0 || parts.length === 0) {
      parts.push(`${seconds} 秒`);
    }

    return parts.join(" ");
  };

  const syncDurationInput = (durationSeconds: number): void => {
    if (durationSeconds % 3600 === 0) {
      amount = durationSeconds / 3600;
      unit = "hours";
      return;
    }

    if (durationSeconds % 60 === 0) {
      amount = durationSeconds / 60;
      unit = "minutes";
      return;
    }

    amount = durationSeconds;
    unit = "seconds";
  };

  const readDurationSeconds = (): number | null => {
    if (!Number.isFinite(amount) || amount <= 0) {
      return null;
    }

    const totalSeconds = Math.round(amount * unitToSeconds(unit));

    if (totalSeconds < MIN_SECONDS || totalSeconds > MAX_SECONDS) {
      return null;
    }

    return totalSeconds;
  };

  const loadSettings = async (): Promise<void> => {
    const settings = await window.workTimer.getSettings();

    syncDurationInput(settings.durationSeconds);
    todayWorkedSeconds = settings.todayWorkedSeconds;
    statusText = `当前每轮 ${formatDuration(settings.durationSeconds)}。`;
  };

  const handleSubmit = async (): Promise<void> => {
    const durationSeconds = readDurationSeconds();

    if (durationSeconds === null) {
      statusText = "请输入 1 秒到 1 小时之间的时间。";
      return;
    }

    isSaving = true;

    try {
      const settings = await window.workTimer.saveDuration(durationSeconds);

      syncDurationInput(settings.durationSeconds);
      todayWorkedSeconds = settings.todayWorkedSeconds;
      statusText = `已保存，每轮 ${formatDuration(settings.durationSeconds)}。`;
    } finally {
      isSaving = false;
    }
  };

  onMount(() => {
    void loadSettings();
  });
</script>

<main
  class="grid min-h-screen place-items-center bg-[#f5f1e8] bg-[linear-gradient(135deg,rgba(37,99,88,0.12),transparent_38%),linear-gradient(315deg,rgba(200,91,71,0.14),transparent_42%)] p-7 max-[430px]:p-5"
>
  <section class="grid w-full max-w-[420px] gap-6" aria-labelledby="app-title">
    <div>
      <p class="mb-1.5 text-[13px] font-bold text-[#587168]">工作计时</p>
      <h1 id="app-title" class="m-0 text-[34px] font-bold leading-[1.1] text-[#17211b]">
        设置工作时间
      </h1>
    </div>

    <div
      class="rounded-lg border border-[#d6d0c3] bg-white/70 px-5 py-7 text-center text-[30px] font-extrabold leading-tight text-[#10382e] [font-variant-numeric:tabular-nums]"
      aria-live="polite"
    >
      <span class="block text-sm font-bold text-[#587168]">今日已工作</span>
      {formatDuration(todayWorkedSeconds)}
    </div>

    <form class="grid grid-cols-[1fr_132px] gap-3.5 max-[430px]:grid-cols-1" on:submit|preventDefault={handleSubmit}>
      <label class="grid gap-2 text-sm font-bold text-[#4d5d56]">
        <span>每轮时长</span>
        <input
          class="min-h-[46px] w-full rounded-md border border-[#cfc7b8] bg-[#fffdf8] px-3 text-[#17211b] disabled:bg-[#ede7dc] disabled:text-[#748078]"
          type="number"
          min="0.0002777778"
          step="any"
          required
          disabled={isSaving}
          bind:value={amount}
        />
      </label>

      <label class="grid gap-2 text-sm font-bold text-[#4d5d56]">
        <span>单位</span>
        <select
          class="min-h-[46px] w-full rounded-md border border-[#cfc7b8] bg-[#fffdf8] px-3 text-[#17211b] disabled:bg-[#ede7dc] disabled:text-[#748078]"
          disabled={isSaving}
          bind:value={unit}
        >
          <option value="seconds">秒</option>
          <option value="minutes">分钟</option>
          <option value="hours">小时</option>
        </select>
      </label>

      <button
        class="col-span-full min-h-12 cursor-pointer rounded-md border-0 bg-[#1e6b5d] font-extrabold text-[#fffdf8] disabled:cursor-default disabled:bg-[#81918b]"
        type="submit"
        disabled={isSaving}
      >
        保存设置
      </button>
    </form>

    <p class="m-0 min-h-[22px] text-sm text-[#66736d]">{statusText}</p>
  </section>
</main>
