import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workTimer", {
  getSettings: () => ipcRenderer.invoke("settings:get") as Promise<{
    durationSeconds: number;
    todayWorkedSeconds: number;
  }>,
  saveDuration: (durationSeconds: number) => ipcRenderer.invoke("settings:save-duration", durationSeconds) as Promise<{
    durationSeconds: number;
    todayWorkedSeconds: number;
  }>
});
