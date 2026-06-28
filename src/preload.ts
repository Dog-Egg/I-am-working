import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workTimer", {
  showContinuePrompt: () => ipcRenderer.invoke("timer:show-continue-prompt") as Promise<boolean>
});
