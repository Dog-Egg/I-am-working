import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workTimer", {
  hideMainWindow: () => ipcRenderer.invoke("timer:hide-main-window") as Promise<void>,
  showMainWindow: () => ipcRenderer.invoke("timer:show-main-window") as Promise<void>,
  showContinuePrompt: () => ipcRenderer.invoke("timer:show-continue-prompt") as Promise<boolean>
});
