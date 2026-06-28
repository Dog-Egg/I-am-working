interface Window {
  workTimer: {
    hideMainWindow: () => Promise<void>;
    showMainWindow: () => Promise<void>;
    showContinuePrompt: () => Promise<boolean>;
  };
  workPrompt: {
    respond: (shouldContinue: boolean) => void;
  };
}
