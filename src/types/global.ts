export interface StateResponse {
  buttonLabel: string;
  durationSeconds: number;
  todayWorkedSeconds: number;
  isActive: boolean;
  activeStartedAt: number | null;
  activeDurationSeconds: number | null;
}

export interface SaveDurationResponse {
  durationSeconds: number;
  todayWorkedSeconds: number;
}
