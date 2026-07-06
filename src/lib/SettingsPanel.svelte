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
  let isCliInstalled = $state<boolean | null>(null);
  let cliAction = $state<"install" | "uninstall" | null>(null);
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

  async function refreshCliStatus() {
    const result = await commands.isCliInstalled();

    if (result.status === "ok") {
      isCliInstalled = result.data;
    } else {
      isCliInstalled = null;
      cliInstallError = result.error;
      console.error("failed to check CLI installation status", result.error);
    }
  }

  onMount(() => {
    let disposed = false;

    void commands.getSettings().then((nextSettings) => {
      if (!disposed) settings = nextSettings;
    });
    void refreshCliStatus();

    return () => {
      disposed = true;
    };
  });

  async function installCli() {
    cliAction = "install";
    cliInstallMessage = "";
    cliInstallError = "";

    try {
      const result = await commands.installCli();

      if (result.status === "ok") {
        isCliInstalled = true;
        cliInstallMessage = `已安装到 ${result.data}`;
      } else {
        cliInstallError = result.error;
        console.error("failed to install CLI", result.error);
      }
    } finally {
      cliAction = null;
    }
  }

  async function uninstallCli() {
    cliAction = "uninstall";
    cliInstallMessage = "";
    cliInstallError = "";

    try {
      const result = await commands.uninstallCli();

      if (result.status === "ok") {
        isCliInstalled = false;
        cliInstallMessage = `已卸载 ${result.data}`;
      } else {
        cliInstallError = result.error;
        console.error("failed to uninstall CLI", result.error);
      }
    } finally {
      cliAction = null;
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
      {:else if isCliInstalled === true}
        <span class="text-xs text-zinc-500">已安装 /usr/local/bin/iaw</span>
      {:else if isCliInstalled === false}
        <span class="text-xs text-zinc-500">未安装 /usr/local/bin/iaw</span>
      {:else}
        <span class="text-xs text-zinc-500">正在检查 /usr/local/bin/iaw</span>
      {/if}
    </div>

    <div class="flex flex-wrap items-center gap-2">
      {#if isCliInstalled}
        <button
          class="rounded-md border border-zinc-700 px-3 py-1.5 text-sm font-medium text-zinc-300 transition hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={cliAction !== null}
          onclick={() => void uninstallCli()}
        >
          {cliAction === "uninstall" ? "卸载中" : "卸载 CLI"}
        </button>
      {:else}
        <button
          class="rounded-md bg-cyan-500 px-3 py-1.5 text-sm font-medium text-zinc-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={isCliInstalled === null || cliAction !== null}
          onclick={() => void installCli()}
        >
          {cliAction === "install" ? "安装中" : "安装 CLI"}
        </button>
      {/if}
    </div>
  </div>
</section>
