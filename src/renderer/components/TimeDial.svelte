<script lang="ts">
  export let value = 1500;
  export let min = 1;
  export let max = 3600;
  export let disabled = false;

  const size = 320;
  const center = size / 2;
  const radius = 125;
  const circumference = 2 * Math.PI * radius;
  const tickMarks = Array.from({ length: 60 }, (_, index) => index);

  let dialElement: HTMLDivElement;
  let isDragging = false;
  let prevAngle: number | null = null;
  let accAngle = 0;

  // 线性分段：1 分钟（60 秒）占表盘 1/12（30°），其余 11/12 给 1 分钟到 60 分钟
  const BREAKPOINT_PROGRESS = 1 / 12;
  const BREAKPOINT_VALUE = 60;

  const progressToValue = (progress: number): number => {
    if (progress <= BREAKPOINT_PROGRESS) {
      return min + (progress / BREAKPOINT_PROGRESS) * (BREAKPOINT_VALUE - min);
    }
    const t = (progress - BREAKPOINT_PROGRESS) / (1 - BREAKPOINT_PROGRESS);
    return BREAKPOINT_VALUE + t * (max - BREAKPOINT_VALUE);
  };

  const valueToProgress = (nextValue: number): number => {
    if (nextValue <= min) return 0;
    if (nextValue >= max) return 1;
    if (nextValue <= BREAKPOINT_VALUE) {
      return (
        ((nextValue - min) / (BREAKPOINT_VALUE - min)) * BREAKPOINT_PROGRESS
      );
    }
    return (
      BREAKPOINT_PROGRESS +
      ((nextValue - BREAKPOINT_VALUE) / (max - BREAKPOINT_VALUE)) *
        (1 - BREAKPOINT_PROGRESS)
    );
  };

  // 分段吸附：<1 分钟按 1 秒一步，>=1 分钟按 60 秒一步
  const snapToStep = (nextValue: number): number => {
    if (nextValue < BREAKPOINT_VALUE) {
      return Math.round(nextValue);
    }
    const step = 60;
    return Math.round(nextValue / step) * step;
  };

  $: progress = Math.min(1, Math.max(0, valueToProgress(value)));
  $: dashOffset = circumference * (1 - progress);
  $: knobAngle = -90 + progress * 360;
  $: knobX = center + Math.cos((knobAngle * Math.PI) / 180) * radius;
  $: knobY = center + Math.sin((knobAngle * Math.PI) / 180) * radius;
  $: displayMinutes = Math.floor(value / 60);
  $: displaySeconds = value % 60;

  const clamp = (nextValue: number): number =>
    Math.min(max, Math.max(min, nextValue));

  const setValueFromPointer = (event: PointerEvent): void => {
    if (disabled || !dialElement) {
      return;
    }

    const rect = dialElement.getBoundingClientRect();
    const x = event.clientX - rect.left - rect.width / 2;
    const y = event.clientY - rect.top - rect.height / 2;
    const angle = Math.atan2(y, x) * (180 / Math.PI);
    const normalizedAngle = (angle + 90 + 360) % 360;

    if (prevAngle !== null) {
      let delta = normalizedAngle - prevAngle;
      if (delta > 180) delta -= 360;
      if (delta < -180) delta += 360;
      accAngle = Math.min(360, Math.max(0, accAngle + delta));
    }
    prevAngle = normalizedAngle;

    const nextValue = snapToStep(progressToValue(accAngle / 360));
    value = clamp(nextValue);
  };

  const handlePointerDown = (event: PointerEvent): void => {
    if (disabled) {
      return;
    }

    const target = event.target as Element | null;
    if (!target || !target.hasAttribute("data-knob")) {
      return;
    }

    isDragging = true;
    dialElement.setPointerCapture(event.pointerId);
    prevAngle = null;
    accAngle = valueToProgress(value) * 360;
    setValueFromPointer(event);
  };

  const handlePointerMove = (event: PointerEvent): void => {
    if (isDragging) {
      setValueFromPointer(event);
    }
  };

  const handlePointerEnd = (event: PointerEvent): void => {
    isDragging = false;

    if (dialElement.hasPointerCapture(event.pointerId)) {
      dialElement.releasePointerCapture(event.pointerId);
    }
  };

  // 键盘步进：按当前值 5% 增减，自然形成非线性步距
  const keyboardStep = (currentValue: number): number =>
    Math.max(1, Math.round(currentValue * 0.05));
</script>

<div
  class="time-dial relative mx-auto aspect-square w-full max-w-[360px] touch-none select-none"
  class:opacity-70={disabled}
  bind:this={dialElement}
  role="slider"
  aria-label="每轮工作时间"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  tabindex={disabled ? -1 : 0}
  on:pointerdown={handlePointerDown}
  on:pointermove={handlePointerMove}
  on:pointerup={handlePointerEnd}
  on:pointercancel={handlePointerEnd}
  on:keydown={(event) => {
    if (disabled) {
      return;
    }

    if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      event.preventDefault();
      value = clamp(value - keyboardStep(value));
    }

    if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      event.preventDefault();
      value = clamp(value + keyboardStep(value));
    }
  }}
>
  <svg
    class="h-full w-full overflow-visible"
    viewBox={`0 0 ${size} ${size}`}
    aria-hidden="true"
  >
    <defs>
      <filter id="dial-glow" x="-40%" y="-40%" width="180%" height="180%">
        <feGaussianBlur stdDeviation="5" result="blur" />
        <feColorMatrix
          in="blur"
          type="matrix"
          values="1 0 0 0 1 0 0 0 0 .28 0 0 0 0 .16 0 0 0 .8 0"
        />
        <feMerge>
          <feMergeNode />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
    </defs>

    <circle
      cx={center}
      cy={center}
      r="141"
      fill="rgba(20, 22, 30, 0.72)"
      stroke="rgba(255, 255, 255, 0.12)"
      stroke-width="2"
    />
    <circle
      cx={center}
      cy={center}
      r="111"
      fill="rgba(19, 20, 28, 0.92)"
      stroke="rgba(255, 255, 255, 0.08)"
      stroke-width="2"
    />

    {#each tickMarks as tick}
      {@const angle = -90 + tick * 6}
      {@const isMajor = tick % 5 === 0}
      <line
        x1={center + Math.cos((angle * Math.PI) / 180) * 132}
        y1={center + Math.sin((angle * Math.PI) / 180) * 132}
        x2={center + Math.cos((angle * Math.PI) / 180) * (isMajor ? 118 : 124)}
        y2={center + Math.sin((angle * Math.PI) / 180) * (isMajor ? 118 : 124)}
        stroke={isMajor
          ? "rgba(255, 128, 96, 0.82)"
          : "rgba(255, 255, 255, 0.56)"}
        stroke-width={isMajor ? 2.4 : 1.7}
        stroke-linecap="round"
      />
    {/each}

    <circle
      cx={center}
      cy={center}
      r={radius}
      fill="none"
      stroke="rgba(255,255,255,0.08)"
      stroke-width="28"
    />
    <circle
      cx={center}
      cy={center}
      r={radius}
      fill="none"
      stroke="#ff6048"
      stroke-opacity="0.7"
      stroke-width="25"
      stroke-dasharray={circumference}
      stroke-dashoffset={dashOffset}
      transform={`rotate(-90 ${center} ${center})`}
    />

    <!-- handler -->
    <circle
      data-knob
      cx={knobX}
      cy={knobY}
      r="17"
      fill="#fffaf2"
      stroke="#ff6a52"
      stroke-width="7"
      style={`cursor: ${disabled ? "not-allowed" : isDragging ? "grabbing" : "grab"};`}
    />
  </svg>

  <div
    class="pointer-events-none absolute inset-0 grid place-items-center text-center"
  >
    <div>
      <div
        class="text-[clamp(58px,10vw,75px)] leading-none font-black tracking-normal text-white [font-variant-numeric:tabular-nums]"
      >
        {String(displayMinutes).padStart(2, "0")}:{String(
          displaySeconds,
        ).padStart(2, "0")}
      </div>
    </div>
  </div>
</div>

<style>
  .no-drag {
    -webkit-app-region: no-drag;
  }
</style>
