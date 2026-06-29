<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { SaveDurationResponse, StateResponse } from "../types/global";
  import TimeDial from "./components/TimeDial.svelte";

  let buttonLabel = "开始工作";
  let durationSeconds = 1500;
  let savedSeconds = 1500;
  let todayWorkedSeconds = 0;
  let isReady = false;
  let isSaving = false;
  let saveTimer: number | null = null;
  let isActive = false;
  let activeStartedAt: number | null = null;
  let activeDurationSeconds: number | null = null;
  let baseWorkedSeconds = 0;
  let tickInterval: number | null = null;
  let finishedUnlisten: UnlistenFn | null = null;

  const toSeconds = (durationSeconds: number): number => {
    if (!Number.isFinite(durationSeconds)) {
      return 1500;
    }

    return Math.min(3600, Math.max(1, Math.round(durationSeconds)));
  };

  const formatWorkedTime = (totalSeconds: number): string => {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = Math.floor(totalSeconds % 60);

    return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  };

  const clearSaveTimer = (): void => {
    if (saveTimer) {
      window.clearTimeout(saveTimer);
      saveTimer = null;
    }
  };

  const computeActiveElapsedSeconds = (): number => {
    if (
      !isActive ||
      activeStartedAt === null ||
      activeDurationSeconds === null
    ) {
      return 0;
    }

    return Math.max(
      0,
      Math.min(
        activeDurationSeconds,
        Math.floor((Date.now() - activeStartedAt) / 1000),
      ),
    );
  };

  const stopTick = (): void => {
    if (tickInterval !== null) {
      window.clearInterval(tickInterval);
      tickInterval = null;
    }
  };

  const startTick = (): void => {
    stopTick();
    todayWorkedSeconds = baseWorkedSeconds + computeActiveElapsedSeconds();
    tickInterval = window.setInterval(() => {
      todayWorkedSeconds = baseWorkedSeconds + computeActiveElapsedSeconds();
    }, 1000);
  };

  const saveDuration = async (): Promise<void> => {
    clearSaveTimer();

    if (durationSeconds === savedSeconds) {
      return;
    }

    isSaving = true;

    try {
      const state = await invoke<SaveDurationResponse>("save_duration", {
        durationSeconds,
      });

      savedSeconds = toSeconds(state.durationSeconds);
      durationSeconds = savedSeconds;
      todayWorkedSeconds = state.todayWorkedSeconds;
    } finally {
      isSaving = false;
    }
  };

  const scheduleSaveDuration = (): void => {
    clearSaveTimer();
    saveTimer = window.setTimeout(() => {
      void saveDuration();
    }, 250);
  };

  const loadState = async (): Promise<void> => {
    const state = await invoke<StateResponse>("get_state");

    buttonLabel = state.buttonLabel;
    durationSeconds = toSeconds(state.durationSeconds);
    savedSeconds = durationSeconds;
    todayWorkedSeconds = state.todayWorkedSeconds;
    baseWorkedSeconds = state.todayWorkedSeconds;
    isActive = state.isActive;
    activeStartedAt = state.activeStartedAt;
    activeDurationSeconds = state.activeDurationSeconds;
    isReady = true;

    if (isActive) {
      startTick();
    } else {
      stopTick();
    }
  };

  const startWork = async (): Promise<void> => {
    await saveDuration();
    await invoke("start_work");
    await loadState();
  };

  $: if (isReady && durationSeconds !== savedSeconds && !isSaving) {
    scheduleSaveDuration();
  }

  onMount(() => {
    void loadState();
    void listen("timer:finished", () => {
      stopTick();
      isActive = false;
      activeStartedAt = null;
      activeDurationSeconds = null;
      void loadState();
    }).then((un) => {
      finishedUnlisten = un;
    });
  });

  onDestroy(() => {
    finishedUnlisten?.();
    clearSaveTimer();
    stopTick();
  });
</script>

<main class="grid min-h-screen place-items-center bg-transparent text-white">
  <section
    data-tauri-drag-region
    class="grid h-full w-full grid-cols-[minmax(210px,0.78fr)_minmax(360px,1.22fr)] gap-8 rounded-[30px] border border-white/10 bg-[#191b24]/95 p-8 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] max-[720px]:grid-cols-1"
    aria-labelledby="app-title"
  >
    <div
      class="flex flex-col rounded-[26px] border border-white/10 bg-white/5.5 p-6 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
    >
      <div
        class="mb-8 grid size-12 place-items-center rounded-full border-2 border-[#ff705c] text-[#ff705c]"
      >
        <svg class="size-7" viewBox="0 0 24 24" aria-hidden="true">
          <circle
            cx="12"
            cy="12"
            r="8"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
          />
          <path
            d="M12 7v5l4 3"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2.2"
          />
        </svg>
      </div>
      <p class="m-0 text-[23px] font-bold text-white/82">今日工作时长</p>
      <p
        class="m-0 mt-4 bg-[linear-gradient(180deg,#fff_20%,#ffd9aa_100%)] bg-clip-text text-[40px] leading-none font-black tracking-normal text-transparent [font-variant-numeric:tabular-nums]"
      >
        {formatWorkedTime(todayWorkedSeconds)}
      </p>

      <div
        class="mt-auto flex h-24 items-end gap-4 border-b border-white/10 pb-2"
      >
        {#each [42, 36, 58, 48, 78, 42, 30, 52] as height}
          <span
            class="block w-4 rounded-full bg-[linear-gradient(180deg,#ff735d,rgba(255,115,93,0.08))]"
            style={`height: ${height}px`}
          ></span>
        {/each}
      </div>
    </div>

    <div class="grid content-center gap-6">
      <TimeDial
        bind:value={durationSeconds}
        min={1}
        max={3600}
        disabled={isSaving}
      />

      <button
        class="mx-auto min-h-16 w-full max-w-130 cursor-pointer rounded-full border border-[#ff937c] bg-[linear-gradient(180deg,#ff7c63_0%,#ff4e3d_100%)] px-8 text-[34px] leading-none font-black text-white shadow-[0_16px_36px_rgba(255,86,64,0.34),inset_0_1px_0_rgba(255,255,255,0.35)] transition hover:brightness-105 active:translate-y-px disabled:cursor-default disabled:opacity-70"
        type="button"
        disabled={isSaving || isActive}
        on:click={startWork}
      >
        {isActive ? "工作中..." : buttonLabel}
      </button>
    </div>
  </section>
</main>

<style>
  :global(html),
  :global(body),
  :global(#app) {
    background: transparent;
  }
</style>
