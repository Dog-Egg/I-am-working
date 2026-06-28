import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workPrompt", {
  getState: () => ipcRenderer.invoke("timer:get-prompt-state") as Promise<{
    buttonLabel: string;
    durationSeconds: number;
    todayWorkedSeconds: number;
  }>,
  saveDuration: (durationSeconds: number) => ipcRenderer.invoke("settings:save-duration", durationSeconds) as Promise<{
    durationSeconds: number;
    todayWorkedSeconds: number;
  }>,
  startWork: () => {
    ipcRenderer.send("timer:start-work");
  }
});
