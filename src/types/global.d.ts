interface Window {
  workTimer: {
    showContinuePrompt: () => Promise<boolean>;
  };
  workPrompt: {
    respond: (shouldContinue: boolean) => void;
  };
}
