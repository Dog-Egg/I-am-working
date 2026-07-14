<script lang="ts">
  import {
    commands,
    type AppSettings,
    type TrayTimeFormat,
  } from "$lib/bindings";
  import { onMount } from "svelte";

  const trayTimeFormatOptions: TrayTimeFormat[] = ["HH:MM", "HH:MM:SS"];

  let settings = $state<AppSettings>({
    show_tray_time: true,
    tray_time_format: "HH:MM",
    launch_at_login: true,
    nosleep_enabled: false,
  });
  let settingsAction = $state(false);
  let nosleepError = $state("");

  async function saveSettings(
    nextSettings: AppSettings,
    reportsNosleepError = false,
  ) {
    if (settingsAction) return;
    settingsAction = true;
    if (reportsNosleepError) nosleepError = "";

    try {
      const result = await commands.updateSettings(nextSettings);

      if (result.status === "ok") {
        settings = result.data;
      } else {
        if (reportsNosleepError) nosleepError = result.error;
        settings = await commands.getSettings();
        console.error("failed to update settings", result.error);
      }
    } finally {
      settingsAction = false;
    }
  }

  onMount(() => {
    let disposed = false;

    void commands.getSettings().then((nextSettings) => {
      if (!disposed) settings = nextSettings;
    });
    return () => {
      disposed = true;
    };
  });

  async function setNosleepEnabled(enabled: boolean) {
    if (enabled === settings.nosleep_enabled) return;
    await saveSettings(
      {
        ...settings,
        nosleep_enabled: enabled,
      },
      true,
    );
  }
</script>

<section
  class="rounded-lg border border-zinc-800 bg-zinc-900/80 px-5 shadow-2xl shadow-black/20"
>
  <div
    class="flex min-h-16 flex-wrap items-center justify-between gap-4 border-b border-zinc-800"
  >
    <div class="flex items-center gap-4">
      <span class="text-sm font-medium text-zinc-300">开机自动运行</span>
      <label class="flex items-center gap-2 text-sm text-zinc-400">
        <input
          class="h-4 w-4 accent-cyan-400"
          type="checkbox"
          checked={settings.launch_at_login}
          disabled={settingsAction}
          onchange={(event) =>
            void saveSettings({
              ...settings,
              launch_at_login: event.currentTarget.checked,
            })}
        />
      </label>
    </div>
  </div>

  <div
    class="flex min-h-16 flex-wrap items-center justify-between gap-4 border-zinc-800"
  >
    <div class="flex items-center gap-4">
      <span class="text-sm font-medium text-zinc-300">托盘显示今日工作时长</span
      >
      <label class="flex items-center gap-2 text-sm text-zinc-400">
        <input
          class="h-4 w-4 accent-cyan-400"
          type="checkbox"
          checked={settings.show_tray_time}
          disabled={settingsAction}
          onchange={(event) =>
            void saveSettings({
              ...settings,
              show_tray_time: event.currentTarget.checked,
            })}
        />
      </label>
    </div>

    <div class="flex flex-wrap items-center gap-3">
      <div
        class="grid grid-cols-2 rounded-lg border border-zinc-700 bg-zinc-950 p-1"
      >
        {#each trayTimeFormatOptions as option}
          <button
            class="rounded-md px-3 py-1 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-40
            {settings.tray_time_format === option
              ? 'bg-cyan-500 text-zinc-950'
              : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
            type="button"
            disabled={settingsAction || !settings.show_tray_time}
            onclick={() =>
              void saveSettings({
                ...settings,
                tray_time_format: option,
              })}
          >
            {option}
          </button>
        {/each}
      </div>
    </div>
  </div>

  <div
    class="flex min-h-16 flex-wrap items-center justify-between gap-4 border-t border-zinc-800 py-3"
  >
    <div class="flex flex-col gap-1">
      <div class="flex items-center gap-1.5">
        <span class="text-sm font-medium text-zinc-300">防止系统休眠</span>
        <span class="group relative inline-flex">
          <button
            class="flex h-4 w-4 items-center justify-center rounded-full border border-zinc-600 text-[10px] font-medium text-zinc-400 transition hover:border-zinc-400 hover:text-zinc-200 focus:border-cyan-400 focus:text-cyan-300 focus:outline-none"
            type="button"
            aria-label="防止系统休眠说明">?</button
          >
          <span
            class="pointer-events-none absolute bottom-full left-1/2 z-10 mb-2 w-72 -translate-x-1/2 rounded-md border border-zinc-700 bg-zinc-950 px-3 py-2 text-xs leading-5 text-zinc-300 opacity-0 shadow-xl transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
            role="tooltip"
            >任务开始：<code>iaw nosleep on --name &lt;name&gt;</code><br />
            任务结束：<code>iaw nosleep off --name &lt;name&gt;</code><br />
            相同任务需使用相同的 name。</span
          >
        </span>
      </div>
      {#if nosleepError}
        <span class="text-xs text-red-300">{nosleepError}</span>
      {/if}
    </div>

    <div
      class="grid grid-cols-2 rounded-lg border border-zinc-700 bg-zinc-950 p-1"
    >
      {#each [true, false] as enabled}
        <button
          class="rounded-md px-3 py-1 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-50
          {settings.nosleep_enabled === enabled
            ? 'bg-cyan-500 text-zinc-950'
            : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
          type="button"
          disabled={settingsAction}
          onclick={() => void setNosleepEnabled(enabled)}
        >
          {enabled ? "开启" : "关闭"}
        </button>
      {/each}
    </div>
  </div>
</section>
