const MIN_SECONDS = 1;
const MAX_SECONDS = 60 * 60;
const DEFAULT_SECONDS = 25 * 60;

type TimeUnit = "seconds" | "minutes" | "hours";

const form = document.querySelector<HTMLFormElement>("#timer-form");
const amountInput = document.querySelector<HTMLInputElement>("#amount");
const unitSelect = document.querySelector<HTMLSelectElement>("#unit");
const startButton = document.querySelector<HTMLButtonElement>("#start-button");
const statusText = document.querySelector<HTMLParagraphElement>("#status");
const display = document.querySelector<HTMLDivElement>("#display");

interface Window {
  workTimer: {
    showContinuePrompt: () => Promise<boolean>;
  };
}

if (!form || !amountInput || !unitSelect || !startButton || !statusText || !display) {
  throw new Error("Missing timer UI elements.");
}

let selectedSeconds = DEFAULT_SECONDS;
let remainingSeconds = DEFAULT_SECONDS;
let intervalId: number | undefined;

const unitToSeconds = (unit: TimeUnit): number => {
  if (unit === "hours") {
    return 60 * 60;
  }

  if (unit === "minutes") {
    return 60;
  }

  return 1;
};

const formatTime = (totalSeconds: number): string => {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  return `${minutes}:${String(seconds).padStart(2, "0")}`;
};

const readDurationSeconds = (): number | null => {
  const amount = amountInput.valueAsNumber;
  const unit = unitSelect.value as TimeUnit;

  if (!Number.isFinite(amount) || amount <= 0) {
    return null;
  }

  const totalSeconds = Math.round(amount * unitToSeconds(unit));

  if (totalSeconds < MIN_SECONDS || totalSeconds > MAX_SECONDS) {
    return null;
  }

  return totalSeconds;
};

const setControlsDisabled = (isDisabled: boolean): void => {
  amountInput.disabled = isDisabled;
  unitSelect.disabled = isDisabled;
  startButton.disabled = isDisabled;
};

const render = (): void => {
  display.textContent = formatTime(remainingSeconds);
};

const stopTimer = (): void => {
  if (intervalId !== undefined) {
    window.clearInterval(intervalId);
    intervalId = undefined;
  }
};

const finishTimer = async (): Promise<void> => {
  stopTimer();
  remainingSeconds = 0;
  render();

  const shouldContinue = await window.workTimer.showContinuePrompt();

  if (shouldContinue) {
    startTimer(selectedSeconds);
    return;
  }

  remainingSeconds = selectedSeconds;
  render();
  setControlsDisabled(false);
  statusText.textContent = "已停止，可以重新设置时间。";
};

const startTimer = (durationSeconds: number): void => {
  stopTimer();
  selectedSeconds = durationSeconds;
  remainingSeconds = durationSeconds;
  setControlsDisabled(true);
  statusText.textContent = "正在工作...";
  render();

  intervalId = window.setInterval(() => {
    remainingSeconds -= 1;
    render();

    if (remainingSeconds <= 0) {
      void finishTimer();
    }
  }, 1000);
};

form.addEventListener("submit", (event) => {
  event.preventDefault();

  const durationSeconds = readDurationSeconds();

  if (durationSeconds === null) {
    statusText.textContent = "请输入 1 秒到 1 小时之间的时间。";
    return;
  }

  startTimer(durationSeconds);
});

render();
