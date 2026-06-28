import { app, BrowserWindow, ipcMain } from "electron";
import * as path from "path";

const showContinuePrompt = (parentWindow: BrowserWindow): Promise<boolean> => {
  return new Promise((resolve) => {
    let isSettled = false;
    const parentOptions = parentWindow.isVisible() ? { parent: parentWindow, modal: true } : {};

    const promptWindow = new BrowserWindow({
      width: 380,
      height: 230,
      resizable: false,
      maximizable: false,
      minimizable: false,
      title: "时间到了",
      alwaysOnTop: true,
      autoHideMenuBar: true,
      ...parentOptions,
      webPreferences: {
        preload: path.join(__dirname, "..", "preload", "prompt-preload.js"),
        contextIsolation: true,
        nodeIntegration: false
      }
    });

    const finish = (shouldContinue: boolean): void => {
      if (isSettled) {
        return;
      }

      isSettled = true;
      ipcMain.removeListener("timer:prompt-response", handlePromptResponse);

      if (!promptWindow.isDestroyed()) {
        promptWindow.close();
      }

      resolve(shouldContinue);
    };

    const handlePromptResponse = (event: Electron.IpcMainEvent, shouldContinue: boolean): void => {
      if (event.sender === promptWindow.webContents) {
        finish(shouldContinue);
      }
    };

    ipcMain.on("timer:prompt-response", handlePromptResponse);

    promptWindow.once("closed", () => {
      finish(false);
    });

    promptWindow.once("ready-to-show", () => {
      promptWindow.setAlwaysOnTop(true, "screen-saver");
      promptWindow.show();
      promptWindow.focus();
    });

    void promptWindow.loadFile(path.join(__dirname, "..", "renderer", "prompt.html"));
  });
};

const createWindow = (): void => {
  const mainWindow = new BrowserWindow({
    width: 520,
    height: 420,
    minWidth: 420,
    minHeight: 360,
    webPreferences: {
      preload: path.join(__dirname, "..", "preload", "app-preload.js"),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  void mainWindow.loadFile(path.join(__dirname, "..", "renderer", "index.html"));
};

app.whenReady().then(() => {
  ipcMain.handle("timer:hide-main-window", (event) => {
    BrowserWindow.fromWebContents(event.sender)?.hide();
  });

  ipcMain.handle("timer:show-main-window", (event) => {
    const mainWindow = BrowserWindow.fromWebContents(event.sender);

    if (!mainWindow) {
      return;
    }

    mainWindow.show();
    mainWindow.focus();
  });

  ipcMain.handle("timer:show-continue-prompt", (event) => {
    const parentWindow = BrowserWindow.fromWebContents(event.sender);

    if (!parentWindow) {
      return false;
    }

    return showContinuePrompt(parentWindow);
  });

  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
