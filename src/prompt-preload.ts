import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("workPrompt", {
  respond: (shouldContinue: boolean) => {
    ipcRenderer.send("timer:prompt-response", shouldContinue);
  }
});
