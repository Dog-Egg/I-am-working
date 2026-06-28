<script lang="ts">
  import { onDestroy } from "svelte";

  const MIN_SECONDS = 1;
  const MAX_SECONDS = 60 * 60;
  const DEFAULT_SECONDS = 25 * 60;

  type TimeUnit = "seconds" | "minutes" | "hours";

  let amount = 25;
  let unit: TimeUnit = "minutes";
  let selectedSeconds = DEFAULT_SECONDS;
  let remainingSeconds = DEFAULT_SECONDS;
  let isRunning = false;
  let statusText = "请输入 1 秒到 1 小时之间的时间。";
  let intervalId: number | undefined;

  const unitToSeconds = (selectedUnit: TimeUnit): number => {
    if (selectedUnit === "hours") {
      return 60 * 60;
    }

    if (selectedUnit === "minutes") {
      return 60;
    }

    return 1;
  };

  const formatTime = (totalSeconds: number): string => {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;

    if (hours > 0) {
      return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
    }

    return `${minutes}:${String(seconds).padStart(2, "0")}`;
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

  const stopTimer = (): void => {
    if (intervalId !== undefined) {
      window.clearInterval(intervalId);
      intervalId = undefined;
    }
  };

  const finishTimer = async (): Promise<void> => {
    stopTimer();
    remainingSeconds = 0;

    const shouldContinue = await window.workTimer.showContinuePrompt();

    if (shouldContinue) {
      startTimer(selectedSeconds);
      return;
    }

    remainingSeconds = selectedSeconds;
    isRunning = false;
    statusText = "已停止，可以重新设置时间。";
  };

  const startTimer = (durationSeconds: number): void => {
    stopTimer();
    selectedSeconds = durationSeconds;
    remainingSeconds = durationSeconds;
    isRunning = true;
    statusText = "正在工作...";

    intervalId = window.setInterval(() => {
      remainingSeconds -= 1;

      if (remainingSeconds <= 0) {
        void finishTimer();
      }
    }, 1000);
  };

  const handleSubmit = (): void => {
    const durationSeconds = readDurationSeconds();

    if (durationSeconds === null) {
      statusText = "请输入 1 秒到 1 小时之间的时间。";
      return;
    }

    startTimer(durationSeconds);
  };

  onDestroy(stopTimer);
</script>

<main
  class="grid min-h-screen place-items-center bg-[#f5f1e8] bg-[linear-gradient(135deg,rgba(37,99,88,0.12),transparent_38%),linear-gradient(315deg,rgba(200,91,71,0.14),transparent_42%)] p-7 max-[430px]:p-5"
>
  <section class="grid w-full max-w-[420px] gap-6" aria-labelledby="app-title">
    <div>
      <p class="mb-1.5 text-[13px] font-bold text-[#587168]">工作计时</p>
      <h1 id="app-title" class="m-0 text-[34px] font-bold leading-[1.1] text-[#17211b]">
        I Am Working
      </h1>
    </div>

    <div
      class="rounded-lg border border-[#d6d0c3] bg-white/70 px-5 py-7 text-center text-[64px] font-extrabold leading-none text-[#10382e] [font-variant-numeric:tabular-nums] max-[430px]:text-[52px]"
      aria-live="polite"
    >
      {formatTime(remainingSeconds)}
    </div>

    <form class="grid grid-cols-[1fr_132px] gap-3.5 max-[430px]:grid-cols-1" on:submit|preventDefault={handleSubmit}>
      <label class="grid gap-2 text-sm font-bold text-[#4d5d56]">
        <span>时长</span>
        <input
          class="min-h-[46px] w-full rounded-md border border-[#cfc7b8] bg-[#fffdf8] px-3 text-[#17211b] disabled:bg-[#ede7dc] disabled:text-[#748078]"
          type="number"
          min="0.0002777778"
          step="any"
          required
          disabled={isRunning}
          bind:value={amount}
        />
      </label>

      <label class="grid gap-2 text-sm font-bold text-[#4d5d56]">
        <span>单位</span>
        <select
          class="min-h-[46px] w-full rounded-md border border-[#cfc7b8] bg-[#fffdf8] px-3 text-[#17211b] disabled:bg-[#ede7dc] disabled:text-[#748078]"
          disabled={isRunning}
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
        disabled={isRunning}
      >
        开始工作
      </button>
    </form>

    <p class="m-0 min-h-[22px] text-sm text-[#66736d]">{statusText}</p>
  </section>
</main>
