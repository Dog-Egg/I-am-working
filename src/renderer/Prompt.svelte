<script lang="ts">
  import { onMount } from "svelte";

  let continueButton: HTMLButtonElement;

  const respond = (shouldContinue: boolean): void => {
    window.workPrompt.respond(shouldContinue);
  };

  onMount(() => {
    continueButton.focus();
  });
</script>

<main
  class="grid min-h-screen content-center gap-3 border-t-[5px] border-[#1e6b5d] bg-[#fffdf8] bg-[linear-gradient(135deg,rgba(30,107,93,0.1),transparent_44%)] p-7 text-[#17211b]"
>
  <p class="m-0 text-[13px] font-extrabold text-[#587168]">工作提醒</p>
  <h1 class="m-0 text-3xl font-bold leading-[1.1] text-[#10382e]">时间到了</h1>
  <p class="mb-2 mt-0 text-base text-[#4d5d56]">是否继续工作？</p>

  <div class="grid grid-cols-2 gap-2.5">
    <button
      class="min-h-11 cursor-pointer rounded-md border border-[#cfc7b8] bg-[#fffdf8] font-extrabold text-[#33413b]"
      type="button"
      on:click={() => respond(false)}
    >
      先暂停
    </button>
    <button
      class="min-h-11 cursor-pointer rounded-md border-0 bg-[#1e6b5d] font-extrabold text-[#fffdf8]"
      type="button"
      bind:this={continueButton}
      on:click={() => respond(true)}
    >
      继续工作
    </button>
  </div>
</main>
