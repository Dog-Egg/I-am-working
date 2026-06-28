const continueButton = document.querySelector<HTMLButtonElement>("#continue-button");
const stopButton = document.querySelector<HTMLButtonElement>("#stop-button");

interface Window {
  workPrompt: {
    respond: (shouldContinue: boolean) => void;
  };
}

if (!continueButton || !stopButton) {
  throw new Error("Missing prompt UI elements.");
}

continueButton.addEventListener("click", () => {
  window.workPrompt.respond(true);
});

stopButton.addEventListener("click", () => {
  window.workPrompt.respond(false);
});
