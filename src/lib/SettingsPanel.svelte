<script lang="ts">
  import { commands, type AppSettings, type TrayTimeFormat } from "$lib/bindings";
  import { onMount } from "svelte";

  const trayTimeFormatOptions: TrayTimeFormat[] = ["HH:MM", "HH:MM:SS"];

  let settings = $state<AppSettings>({
    show_tray_time: true,
    tray_time_format: "HH:MM",
    launch_at_login: true,
  });
  let settingsRequestId = 0;

  async function saveSettings(nextSettings: AppSettings) {
    const requestId = ++settingsRequestId;
    settings = nextSettings;

    const result = await commands.updateSettings(nextSettings);

    if (result.status === "ok") {
      if (requestId === settingsRequestId) {
        settings = result.data;
      }
    } else {
      if (requestId === settingsRequestId) {
        settings = await commands.getSettings();
      }
      console.error("failed to update settings", result.error);
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
</script>

<section
  class="rounded-lg border border-zinc-800 bg-zinc-900/80 p-5 shadow-2xl shadow-black/20"
>
  <div
    class="mb-5 flex flex-wrap items-center justify-between gap-4 border-zinc-800"
  >
    <div class="flex items-center gap-4">
      <span class="text-sm font-medium text-zinc-300">开机自动运行</span>
      <label class="flex items-center gap-2 text-sm text-zinc-400">
        <input
          class="h-4 w-4 accent-cyan-400"
          type="checkbox"
          checked={settings.launch_at_login}
          onchange={(event) =>
            void saveSettings({
              ...settings,
              launch_at_login: event.currentTarget.checked,
            })}
        />
      </label>
    </div>
  </div>

  <div class="flex flex-wrap items-center justify-between gap-4 border-zinc-800">
    <div class="flex items-center gap-4">
      <span class="text-sm font-medium text-zinc-300"
        >托盘显示今日工作时长</span
      >
      <label class="flex items-center gap-2 text-sm text-zinc-400">
        <input
          class="h-4 w-4 accent-cyan-400"
          type="checkbox"
          checked={settings.show_tray_time}
          onchange={(event) =>
            void saveSettings({
              ...settings,
              show_tray_time: event.currentTarget.checked,
            })}
        />
      </label>
    </div>

    <div class="flex flex-wrap items-center gap-3">
      <div class="grid grid-cols-2 rounded-lg border border-zinc-700 bg-zinc-950 p-1">
        {#each trayTimeFormatOptions as option}
          <button
            class="rounded-md px-3 py-1 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-40
            {settings.tray_time_format === option
              ? 'bg-cyan-500 text-zinc-950'
              : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100'}"
            type="button"
            disabled={!settings.show_tray_time}
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
</section>
