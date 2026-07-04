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
  });
  let settingsRequestId = 0;
  let isInstallingCli = $state(false);
  let cliInstallMessage = $state("");
  let cliInstallError = $state("");

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

  async function installCli() {
    isInstallingCli = true;
    cliInstallMessage = "";
    cliInstallError = "";

    const result = await commands.installCli();
    isInstallingCli = false;

    if (result.status === "ok") {
      cliInstallMessage = `已安装到 ${result.data}`;
    } else {
      cliInstallError = result.error;
      console.error("failed to install CLI", result.error);
    }
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

  <div
    class="flex min-h-16 flex-wrap items-center justify-between gap-4 border-t border-zinc-800 py-3"
  >
    <div class="flex flex-col gap-1">
      <span class="text-sm font-medium text-zinc-300">命令行工具</span>
      {#if cliInstallMessage}
        <span class="text-xs text-emerald-300">{cliInstallMessage}</span>
      {:else if cliInstallError}
        <span class="text-xs text-red-300">{cliInstallError}</span>
      {:else}
        <span class="text-xs text-zinc-500">/usr/local/bin/iaw</span>
      {/if}
    </div>

    <button
      class="rounded-md bg-cyan-500 px-3 py-1.5 text-sm font-medium text-zinc-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
      type="button"
      disabled={isInstallingCli}
      onclick={() => void installCli()}
    >
      {isInstallingCli ? "安装中" : "安装 CLI"}
    </button>
  </div>
</section>
