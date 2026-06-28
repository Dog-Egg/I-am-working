import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workPrompt", {
  getState: () => ipcRenderer.invoke("timer:get-prompt-state") as Promise<{
    buttonLabel: string;
    durationSeconds: number;
    todayWorkedSeconds: number;
  }>,
  startWork: () => {
    ipcRenderer.send("timer:start-work");
  }
});
