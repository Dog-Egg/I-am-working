import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workApi", {
  getState: () =>
    ipcRenderer.invoke("timer:get-state") as Promise<{
      buttonLabel: string;
      durationSeconds: number;
      todayWorkedSeconds: number;
      isActive: boolean;
      activeStartedAt: number | null;
      activeDurationSeconds: number | null;
    }>,
  saveDuration: (durationSeconds: number) =>
    ipcRenderer.invoke("settings:save-duration", durationSeconds) as Promise<{
      durationSeconds: number;
      todayWorkedSeconds: number;
    }>,
  startWork: () => {
    ipcRenderer.send("timer:start-work");
  },
  onFinished: (callback: () => void) => {
    ipcRenderer.on("timer:finished", () => callback());
  },
});
