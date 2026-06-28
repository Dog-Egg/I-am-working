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
  class="grid min-h-screen place-items-center bg-transparent p-3 text-[#17211b]"
>
  <section class="drag-region grid w-full gap-3 bg-transparent">
    <p class="m-0 text-[13px] font-extrabold text-[#587168]">工作提醒</p>
    <h1 class="m-0 text-3xl font-bold leading-[1.1] text-[#10382e]">
      时间到了
    </h1>
    <p class="mb-2 mt-0 text-base text-[#4d5d56]">是否继续工作？</p>

    <div class="grid grid-cols-2 gap-2.5">
      <button
        class="no-drag min-h-11 cursor-pointer rounded-md border border-[#cfc7b8] bg-[#fffdf8] font-extrabold text-[#33413b]"
        type="button"
        on:click={() => respond(false)}
      >
        先暂停
      </button>
      <button
        class="no-drag min-h-11 cursor-pointer rounded-md border-0 bg-[#1e6b5d] font-extrabold text-[#fffdf8]"
        type="button"
        bind:this={continueButton}
        on:click={() => respond(true)}
      >
        继续工作
      </button>
    </div>
  </section>
</main>

<style>
  :global(html),
  :global(body),
  :global(#prompt) {
    background: transparent;
  }

  .drag-region {
    -webkit-app-region: drag;
  }

  .no-drag {
    -webkit-app-region: no-drag;
  }
</style>
