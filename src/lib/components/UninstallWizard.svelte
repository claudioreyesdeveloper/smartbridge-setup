<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getHelpByEmail, uninstallAll } from "$lib/api";
  import { text, type Locale } from "$lib/i18n/messages";
  import type { HelpEmailOutcome } from "$lib/types";

  interface Props {
    locale: Locale;
  }

  type Screen = "confirm" | "choice" | "removing" | "done" | "error";

  let { locale }: Props = $props();
  let screen = $state<Screen>("confirm");
  let keepUserData = $state(true);
  let helpBusy = $state(false);
  let helpOutcome = $state<HelpEmailOutcome | null>(null);

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  async function removeNow() {
    screen = "removing";
    helpOutcome = null;
    try {
      const outcome = await uninstallAll(keepUserData);
      screen = outcome.success ? "done" : "error";
    } catch {
      screen = "error";
    }
  }

  async function handleHelp() {
    helpBusy = true;
    try {
      helpOutcome = await getHelpByEmail();
    } finally {
      helpBusy = false;
    }
  }
</script>

{#if screen === "confirm"}
  <section class="card">
    <h1>{text(locale, "uninstall_confirm_lead")}</h1>
    <p class="lead">{text(locale, "uninstall_confirm_body")}</p>
    <div class="btn-row btn-row-h">
      <button class="btn btn-secondary" onclick={closeWindow}>{text(locale, "generic_cancel")}</button>
      <button class="btn btn-danger" onclick={() => screen = "choice"}>
        {text(locale, "uninstall_confirm_cta")}
      </button>
    </div>
  </section>
{:else if screen === "choice"}
  <section class="card">
    <h1>{text(locale, "uninstall_keep_lead")}</h1>
    <p class="lead">{text(locale, "uninstall_keep_q")}</p>

    <button class:selected={keepUserData} class="choice" onclick={() => keepUserData = true}>
      <span class="choice-radio"></span>
      <span class="choice-body">
        <span class="choice-title">{text(locale, "uninstall_keep_yes")}</span>
        <span class="choice-sub">{text(locale, "uninstall_keep_yes_sub")}</span>
      </span>
    </button>

    <button class:selected={!keepUserData} class="choice" onclick={() => keepUserData = false}>
      <span class="choice-radio"></span>
      <span class="choice-body">
        <span class="choice-title">{text(locale, "uninstall_keep_no")}</span>
        <span class="choice-sub">{text(locale, "uninstall_keep_no_sub")}</span>
      </span>
    </button>

    <div class="btn-row btn-row-h">
      <button class="btn btn-secondary" onclick={() => screen = "confirm"}>
        {text(locale, "generic_back")}
      </button>
      <button class="btn btn-danger" onclick={removeNow}>
        {text(locale, "uninstall_confirm_cta")}
      </button>
    </div>
  </section>
{:else if screen === "removing"}
  <section class="card">
    <h1>{text(locale, "uninstall_progress_lead")}</h1>
    <p class="lead">{text(locale, "uninstall_progress_body")}</p>
    <div class="progress-track" aria-hidden="true">
      <div class="progress-fill indeterminate"></div>
    </div>
  </section>
{:else if screen === "done"}
  <section class="card done">
    <div class="big-check">✓</div>
    <h1>{text(locale, "uninstall_done_lead")}</h1>
    <p class="lead">{text(locale, "uninstall_done_body")}</p>
    <div class="btn-row">
      <button class="btn btn-primary btn-block" onclick={closeWindow}>
        {text(locale, "done_cta_close")}
      </button>
    </div>
  </section>
{:else if screen === "error"}
  <section class="card">
    <div class="big-cross">×</div>
    <h1>{text(locale, "uninstall_failed_lead")}</h1>
    <p class="lead">{text(locale, "uninstall_failed_body")}</p>
    {#if helpOutcome}
      <div class="alert">
        <strong>{text(locale, "error_help_lead")}</strong>
        <p>{text(locale, "error_help_body")}</p>
        <p class="selectable">
          {text(locale, "error_help_address_intro")} {helpOutcome.help_email}
        </p>
        <p class="selectable">{helpOutcome.bundle_path}</p>
      </div>
    {/if}
    <div class="btn-row">
      <button class="btn btn-secondary btn-block" onclick={handleHelp} disabled={helpBusy}>
        {helpBusy ? text(locale, "generic_loading") : text(locale, "error_cta_help")}
      </button>
      <button class="btn btn-primary btn-block" onclick={closeWindow}>
        {text(locale, "error_close")}
      </button>
    </div>
  </section>
{/if}

<style>
  .done {
    text-align: center;
  }
</style>
