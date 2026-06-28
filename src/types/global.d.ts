interface Window {
  workApi: {
    getState: () => Promise<{
      buttonLabel: string;
      durationSeconds: number;
      todayWorkedSeconds: number;
    }>;
    saveDuration: (durationSeconds: number) => Promise<{
      durationSeconds: number;
      todayWorkedSeconds: number;
    }>;
    startWork: () => void;
  };
}
