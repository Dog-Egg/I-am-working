interface Window {
  workApi: {
    getState: () => Promise<{
      buttonLabel: string;
      durationSeconds: number;
      todayWorkedSeconds: number;
      isActive: boolean;
      activeStartedAt: number | null;
      activeDurationSeconds: number | null;
    }>;
    saveDuration: (durationSeconds: number) => Promise<{
      durationSeconds: number;
      todayWorkedSeconds: number;
    }>;
    startWork: () => void;
    onFinished: (callback: () => void) => void;
  };
}
