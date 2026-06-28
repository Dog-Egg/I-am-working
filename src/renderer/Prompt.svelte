<script lang="ts">
  import { onMount } from "svelte";

  let buttonLabel = "开始工作";
  let durationSeconds = 25 * 60;
  let todayWorkedSeconds = 0;

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

  const loadState = async (): Promise<void> => {
    const state = await window.workPrompt.getState();

    buttonLabel = state.buttonLabel;
    durationSeconds = state.durationSeconds;
    todayWorkedSeconds = state.todayWorkedSeconds;
  };

  const startWork = (): void => {
    window.workPrompt.startWork();
  };

  onMount(() => {
    void loadState();
  });
</script>

<main
  class="grid min-h-screen place-items-center bg-transparent p-3 text-[#17211b]"
>
  <section class="drag-region grid w-full gap-3 bg-transparent">
    <p class="m-0 text-[13px] font-extrabold text-[#587168]">工作提醒</p>
    <h1 class="m-0 text-3xl font-bold leading-[1.1] text-[#10382e]">
      I Am Working
    </h1>
    <div class="grid gap-1 text-[15px] font-bold text-[#4d5d56]">
      <p class="m-0">今日已工作：{formatDuration(todayWorkedSeconds)}</p>
      <p class="m-0">本轮时长：{formatDuration(durationSeconds)}</p>
    </div>
    <div class="grid">
      <button
        class="no-drag min-h-11 cursor-pointer rounded-md border-0 bg-[#1e6b5d] font-extrabold text-[#fffdf8]"
        type="button"
        on:click={startWork}
      >
        {buttonLabel}
      </button>
    </div>
  </section>
</main>

<style>
  :global(html),
  :global(body),
  :global(#prompt) {
    background: transparent;
  }

  .drag-region {
    -webkit-app-region: drag;
  }

  .no-drag {
    -webkit-app-region: no-drag;
  }
</style>
