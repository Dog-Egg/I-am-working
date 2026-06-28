import { app, BrowserWindow, Menu, Tray, ipcMain, nativeImage } from "electron";
import { mkdir, readFile, writeFile } from "fs/promises";
import * as path from "path";

const MIN_SECONDS = 1;
const MAX_SECONDS = 60 * 60;
const DEFAULT_SECONDS = 25 * 60;

type DailyWork = {
  workedSeconds: number;
};

type WorkState = {
  version: 1;
  settings: {
    durationSeconds: number;
  };
  dailyWork: Record<string, DailyWork>;
};

type ActiveTimer = {
  startedAt: number;
  durationSeconds: number;
  timeoutId: NodeJS.Timeout;
};

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let activeTimer: ActiveTimer | null = null;
let trayCountdownInterval: NodeJS.Timeout | null = null;
let isQuitting = false;

let workState: WorkState = {
  version: 1,
  settings: {
    durationSeconds: DEFAULT_SECONDS,
  },
  dailyWork: {},
};

const formatDateKey = (date: Date): string => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");

  return `${year}-${month}-${day}`;
};

const getTodayKey = (): string => formatDateKey(new Date());

const getStateFilePath = (): string =>
  path.join(app.getPath("userData"), "work-state.json");

const clampDuration = (durationSeconds: number): number => {
  if (!Number.isFinite(durationSeconds)) {
    return DEFAULT_SECONDS;
  }

  return Math.min(
    MAX_SECONDS,
    Math.max(MIN_SECONDS, Math.round(durationSeconds)),
  );
};

const readWorkedSeconds = (dateKey = getTodayKey()): number =>
  workState.dailyWork[dateKey]?.workedSeconds ?? 0;

const formatCountdown = (totalSeconds: number): string => {
  const seconds = Math.max(0, totalSeconds);
  const minutesPart = Math.floor(seconds / 60);
  const secondsPart = String(seconds % 60).padStart(2, "0");

  return `${minutesPart}:${secondsPart}`;
};

const readActiveTimerRemainingSeconds = (): number => {
  if (!activeTimer) {
    return 0;
  }

  const endsAt = activeTimer.startedAt + activeTimer.durationSeconds * 1000;

  return Math.max(0, Math.ceil((endsAt - Date.now()) / 1000));
};

const readActiveTimerElapsedSeconds = (): number => {
  if (!activeTimer) {
    return 0;
  }

  return activeTimer.durationSeconds - readActiveTimerRemainingSeconds();
};

const readDisplayedWorkedSeconds = (dateKey = getTodayKey()): number =>
  readWorkedSeconds(dateKey) + readActiveTimerElapsedSeconds();

const ensureDailyWork = (dateKey: string): DailyWork => {
  workState.dailyWork[dateKey] ??= { workedSeconds: 0 };

  return workState.dailyWork[dateKey];
};

const loadWorkState = async (): Promise<void> => {
  const filePath = getStateFilePath();

  try {
    const fileContent = await readFile(filePath, "utf8");

    console.log(
      `[IO ${new Date().toISOString()}] READ ${filePath} (${Buffer.byteLength(fileContent, "utf8")} bytes)`,
    );

    const storedState = JSON.parse(fileContent) as Partial<WorkState>;

    workState = {
      version: 1,
      settings: {
        durationSeconds: clampDuration(
          storedState.settings?.durationSeconds ?? DEFAULT_SECONDS,
        ),
      },
      dailyWork: storedState.dailyWork ?? {},
    };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      console.log(
        `[IO ${new Date().toISOString()}] READ ${filePath} (not found, using defaults)`,
      );
    } else {
      console.error("Failed to load work state:", error);
    }
  }
};

const saveWorkState = async (): Promise<void> => {
  const filePath = getStateFilePath();
  const serialized = `${JSON.stringify(workState, null, 2)}\n`;

  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, serialized, "utf8");

  console.log(
    `[IO ${new Date().toISOString()}] WRITE ${filePath} (${Buffer.byteLength(serialized, "utf8")} bytes)`,
  );
};

const addWorkedPeriod = (startedAt: number, durationSeconds: number): void => {
  let remainingSeconds = durationSeconds;
  let cursor = new Date(startedAt);

  while (remainingSeconds > 0) {
    const dateKey = formatDateKey(cursor);
    const nextDay = new Date(cursor);
    nextDay.setHours(24, 0, 0, 0);

    const secondsUntilNextDay = Math.max(
      1,
      Math.ceil((nextDay.getTime() - cursor.getTime()) / 1000),
    );
    const secondsToAdd = Math.min(remainingSeconds, secondsUntilNextDay);

    ensureDailyWork(dateKey).workedSeconds += secondsToAdd;
    remainingSeconds -= secondsToAdd;
    cursor = new Date(cursor.getTime() + secondsToAdd * 1000);
  }
};

const getState = () => {
  const todayWorkedSeconds = readWorkedSeconds();

  return {
    buttonLabel: todayWorkedSeconds === 0 ? "开始工作" : "继续工作",
    durationSeconds: workState.settings.durationSeconds,
    todayWorkedSeconds,
    isActive: !!activeTimer,
    activeStartedAt: activeTimer?.startedAt ?? null,
    activeDurationSeconds: activeTimer?.durationSeconds ?? null,
  };
};

const closeMainWindow = (): void => {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.close();
  }
};

const showMainWindow = (): void => {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.setAlwaysOnTop(true, "screen-saver");
    mainWindow.show();
    mainWindow.focus();
    return;
  }

  mainWindow = new BrowserWindow({
    width: 820,
    height: 620,
    minWidth: 720,
    minHeight: 540,
    resizable: true,
    maximizable: true,
    minimizable: false,
    title: "工作提醒",
    frame: false,
    hasShadow: false,
    transparent: true,
    backgroundColor: "#00000000",
    show: false,
    alwaysOnTop: true,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "..", "preload", "index.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  mainWindow.once("closed", () => {
    mainWindow = null;
  });

  mainWindow.once("ready-to-show", () => {
    mainWindow?.setAlwaysOnTop(true, "screen-saver");
    mainWindow?.show();
    mainWindow?.focus();
  });

  const rendererUrl = process.env["ELECTRON_RENDERER_URL"];

  if (rendererUrl) {
    void mainWindow.loadURL(`${rendererUrl}/index.html`);
  } else {
    void mainWindow.loadFile(
      path.join(__dirname, "..", "renderer", "index.html"),
    );
  }
};

const beginWork = (): void => {
  if (activeTimer) {
    return;
  }

  const durationSeconds = workState.settings.durationSeconds;
  const startedAt = Date.now();

  activeTimer = {
    durationSeconds,
    startedAt,
    timeoutId: setTimeout(() => {
      void finishWork();
    }, durationSeconds * 1000),
  };

  closeMainWindow();
  startTrayCountdown();
  updateTrayMenu();
};

const finishWork = async (): Promise<void> => {
  if (!activeTimer) {
    return;
  }

  const finishedTimer = activeTimer;
  activeTimer = null;
  stopTrayCountdown();

  addWorkedPeriod(finishedTimer.startedAt, finishedTimer.durationSeconds);
  await saveWorkState();
  updateTrayMenu();
  showMainWindow();

  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send("timer:finished");
  }
};

const createTrayIcon = (): Electron.NativeImage => {
  const size = 16;
  const buffer = Buffer.alloc(size * size * 4);

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const offset = (y * size + x) * 4;
      const distance = Math.hypot(x - 7.5, y - 7.5);
      const isCircle = distance <= 6.5;

      buffer[offset] = 30;
      buffer[offset + 1] = 107;
      buffer[offset + 2] = 93;
      buffer[offset + 3] = isCircle ? 255 : 0;
    }
  }

  return nativeImage.createFromBitmap(buffer, { width: size, height: size });
};

const updateTrayMenu = (): void => {
  if (!tray) {
    return;
  }

  const todayMinutes = Math.floor(readDisplayedWorkedSeconds() / 60);
  const remainingSeconds = readActiveTimerRemainingSeconds();
  const countdownLabel = formatCountdown(remainingSeconds);

  tray.setTitle(activeTimer ? countdownLabel : "");
  tray.setToolTip(
    activeTimer
      ? `I Am Working - 本轮剩余 ${countdownLabel}，今日已工作 ${todayMinutes} 分钟`
      : `I Am Working - 今日已工作 ${todayMinutes} 分钟`,
  );
  tray.setContextMenu(
    Menu.buildFromTemplate([
      {
        label: "显示窗口",
        click: showMainWindow,
      },
      {
        label: "退出",
        click: () => {
          isQuitting = true;
          app.quit();
        },
      },
    ]),
  );
};

const startTrayCountdown = (): void => {
  if (trayCountdownInterval) {
    return;
  }

  trayCountdownInterval = setInterval(updateTrayMenu, 1000);
};

const stopTrayCountdown = (): void => {
  if (!trayCountdownInterval) {
    return;
  }

  clearInterval(trayCountdownInterval);
  trayCountdownInterval = null;
};

const createTray = (): void => {
  tray = new Tray(createTrayIcon());
  updateTrayMenu();
};

app.whenReady().then(async () => {
  app.setActivationPolicy("accessory");
  await loadWorkState();

  ipcMain.handle(
    "settings:save-duration",
    async (_event, durationSeconds: number) => {
      workState.settings.durationSeconds = clampDuration(durationSeconds);
      await saveWorkState();
      updateTrayMenu();

      return {
        durationSeconds: workState.settings.durationSeconds,
        todayWorkedSeconds: readWorkedSeconds(),
      };
    },
  );

  ipcMain.handle("timer:get-state", getState);

  ipcMain.on("timer:start-work", (event) => {
    if (mainWindow?.webContents === event.sender) {
      beginWork();
    }
  });

  createTray();
  showMainWindow();

  app.on("activate", () => {
    showMainWindow();
  });
});

app.on("before-quit", () => {
  isQuitting = true;

  if (activeTimer) {
    clearTimeout(activeTimer.timeoutId);
    activeTimer = null;
  }

  stopTrayCountdown();
});

app.on("window-all-closed", () => {
  // Keep the app alive in the tray.
});
