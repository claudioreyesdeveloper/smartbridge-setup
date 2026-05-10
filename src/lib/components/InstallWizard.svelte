<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import {
    activateBeta,
    getHelpByEmail,
    getHostInfo,
    getInstallPlan,
    getLicenseStatus,
    installAll,
  } from "$lib/api";
  import { fill, text, type Locale } from "$lib/i18n/messages";
  import type {
    DownloadProgress,
    HelpEmailOutcome,
    HostInfo,
    InstallPlan,
    LicenseStatus,
    PlanStep,
    ProfileChoice,
    WizardStepEvent,
  } from "$lib/types";

  interface Props {
    locale: Locale;
    onLocaleChange: (locale: Locale) => void;
  }

  type Screen =
    | "loading"
    | "welcome"
    | "activate"
    | "profile"
    | "ready"
    | "installing"
    | "done"
    | "error";

  let { locale, onLocaleChange }: Props = $props();

  let screen = $state<Screen>("loading");
  let license = $state<LicenseStatus | null>(null);
  let host = $state<HostInfo | null>(null);
  let email = $state("");
  let serial = $state("");
  let activationError = $state("");
  let profile = $state<ProfileChoice>({
    use_ai_lyrics: false,
  });
  let plan = $state<InstallPlan | null>(null);
  let currentStep = $state<WizardStepEvent | null>(null);
  let installError = $state("");
  let helpOutcome = $state<HelpEmailOutcome | null>(null);
  let helpBusy = $state(false);
  let unlistenStep: UnlistenFn | null = null;
  let unlistenDownload: UnlistenFn | null = null;
  let downloads = $state<Record<string, DownloadProgress>>({});

  const isMac = $derived(host?.os === "macos");
  const doneBody = $derived(
    isMac ? text(locale, "done_body_macos") : text(locale, "done_body_windows")
  );
  const planSteps = $derived(plan?.steps ?? []);
  const currentDownload = $derived(
    currentStep
      ? Object.values(downloads)
          .filter((p) => p.download_id.startsWith(currentStep!.component_id))
          .sort((a, b) => phaseRank(b.phase) - phaseRank(a.phase))[0] ?? null
      : null
  );
  const currentStepText = $derived(
    currentStep
      ? stepLabel(
          planSteps.find((step) => step.component_id === currentStep!.component_id) ?? {
            component_id: currentStep.component_id,
            label_en: "Working on SmartBridge",
            label_de: "SmartBridge wird eingerichtet",
          }
        )
      : text(locale, "generic_loading")
  );
  const currentPhaseText = $derived(phaseText());
  const currentStepFraction = $derived(
    currentDownload && currentDownload.bytes_total > 0
      ? Math.min(0.95, currentDownload.bytes_downloaded / currentDownload.bytes_total)
      : currentStep?.status === "ok"
        ? 1
        : 0
  );
  const progressIsIndeterminate = $derived(
    screen === "installing" &&
      !!currentStep &&
      currentStep.status === "starting" &&
      !currentDownload
  );
  const progressPercent = $derived(
    currentStep
      ? Math.min(
          100,
          Math.round(
            ((currentStep.step_index + currentStepFraction) /
              Math.max(currentStep.step_count, 1)) *
              100
          )
        )
      : 0
  );

  onMount(async () => {
    unlistenStep = await listen<WizardStepEvent>("wizard://step", (event) => {
      currentStep = event.payload;
    });
    unlistenDownload = await listen<DownloadProgress>(
      "download://progress",
      (event) => {
        const p = event.payload;
        downloads = { ...downloads, [p.download_id]: p };
      }
    );

    try {
      [license, host] = await Promise.all([getLicenseStatus(), getHostInfo()]);
      if (license.flavor === "beta_0_1" && !license.activated) {
        screen = "activate";
      } else {
        screen = "welcome";
      }
    } catch (e) {
      installError = friendlyLoadError(e);
      screen = "error";
    }
  });

  onDestroy(() => {
    unlistenStep?.();
    unlistenDownload?.();
  });

  function stepLabel(step: PlanStep) {
    return locale === "de" ? step.label_de : step.label_en;
  }

  function phaseRank(phase: DownloadProgress["phase"]) {
    switch (phase) {
      case "verified":
      case "verified_local":
      case "cache_hit":
        return 3;
      case "downloading":
      case "verifying_local":
        return 2;
      case "starting":
      default:
        return 1;
    }
  }

  function formatBytes(bytes: number) {
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
    }
    if (bytes >= 1024 * 1024) {
      return `${Math.round(bytes / (1024 * 1024))} MB`;
    }
    if (bytes >= 1024) {
      return `${Math.round(bytes / 1024)} KB`;
    }
    return `${bytes} bytes`;
  }

  function phaseText() {
    if (!currentStep) return text(locale, "generic_loading");

    if (!currentDownload) {
      return locale === "de" ? "Jetzt vorbereiten..." : "Now getting ready...";
    }

    switch (currentDownload.phase) {
      case "starting":
        return locale === "de" ? "Download wird vorbereitet..." : "Now preparing the download...";
      case "downloading":
        return locale === "de" ? "Jetzt herunterladen..." : "Now downloading...";
      case "verifying_local":
        return locale === "de" ? "Lokale Datei wird geprüft..." : "Now checking the local file...";
      case "verified":
      case "verified_local":
      case "cache_hit":
        if (isMac && currentStep.component_id === "main-app") {
          return locale === "de"
            ? "Jetzt installieren. macOS fragt eventuell nach Ihrem Passwort."
            : "Now installing. macOS may ask for your password.";
        }
        return locale === "de" ? "Jetzt installieren..." : "Now installing...";
      default:
        return locale === "de" ? "Jetzt arbeiten..." : "Now working...";
    }
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  async function submitActivation() {
    activationError = "";
    const outcome = await activateBeta(email, serial);
    license = outcome.status;
    if (outcome.ok) {
      screen = "welcome";
    } else {
      activationError = text(locale, "activate_invalid");
    }
  }

  async function preparePlan() {
    plan = await getInstallPlan(profile);
    screen = "ready";
  }

  async function beginInstall() {
    currentStep = null;
    downloads = {};
    installError = "";
    helpOutcome = null;
    screen = "installing";
    try {
      const outcome = await installAll(profile);
      if (outcome.success) {
        screen = "done";
      } else {
        installError = outcome.failure_message || text(locale, "error_body");
        screen = "error";
      }
    } catch (e) {
      installError = friendlyLoadError(e);
      screen = "error";
    }
  }

  async function handleHelp() {
    helpBusy = true;
    try {
      helpOutcome = await getHelpByEmail();
    } catch (e) {
      installError = friendlyLoadError(e);
    } finally {
      helpBusy = false;
    }
  }

  function friendlyLoadError(e: unknown) {
    const raw = String(e).toLowerCase();
    if (raw.includes("network") || raw.includes("fetch") || raw.includes("dns")) {
      return "We could not reach the internet. Please check your connection and try again.";
    }
    return text(locale, "error_body");
  }
</script>

{#if screen === "loading"}
  <section class="card">
    <h1>{text(locale, "generic_loading")}</h1>
    <div class="progress-track" aria-hidden="true">
      <div class="progress-fill indeterminate"></div>
    </div>
  </section>
{:else if screen === "welcome"}
  <section class="card">
    <h1>{text(locale, "welcome_lead")}</h1>
    <p class="lead">{text(locale, "welcome_body")}</p>
    <div class="btn-row">
      <button class="btn btn-primary btn-block" onclick={() => screen = "profile"}>
        {text(locale, "welcome_cta_start")}
      </button>
    </div>
  </section>
{:else if screen === "activate"}
  <section class="card">
    <h1>{text(locale, "activate_lead")}</h1>
    <p class="lead">{text(locale, "activate_body")}</p>
    {#if activationError}
      <div class="alert alert-error">{activationError}</div>
    {/if}
    <div class="field">
      <label class="field-label" for="email">{text(locale, "activate_email_label")}</label>
      <input id="email" class="input" bind:value={email} autocomplete="email" />
    </div>
    <div class="field">
      <label class="field-label" for="serial">{text(locale, "activate_serial_label")}</label>
      <input id="serial" class="input" bind:value={serial} autocomplete="off" />
    </div>
    <div class="btn-row btn-row-h">
      <button class="btn btn-secondary" onclick={closeWindow}>{text(locale, "activate_quit")}</button>
      <button class="btn btn-primary" onclick={submitActivation}>{text(locale, "activate_cta")}</button>
    </div>
  </section>
{:else if screen === "profile"}
  <section class="card">
    <h1>{text(locale, "profile_lead")}</h1>
    <p class="lead">{text(locale, "profile_body")}</p>

    <button
      class:selected={profile.use_ai_lyrics}
      class="choice"
      onclick={() => profile.use_ai_lyrics = !profile.use_ai_lyrics}
    >
      <span class="choice-radio"></span>
      <span class="choice-body">
        <span class="choice-title">{text(locale, "profile_q_ai")}</span>
        <span class="choice-sub">{text(locale, "profile_q_ai_sub")}</span>
      </span>
    </button>

    <div class="btn-row">
      <button class="btn btn-primary btn-block" onclick={preparePlan}>
        {text(locale, "profile_cta")}
      </button>
    </div>
  </section>
{:else if screen === "ready"}
  <section class="card">
    <h1>{text(locale, "ready_lead")}</h1>
    <p class="lead">
      {planSteps.length === 1
        ? text(locale, "ready_body_one")
        : fill(text(locale, "ready_body_many"), { count: planSteps.length })}
    </p>
    <details>
      <summary>{text(locale, "ready_what")}</summary>
      <ul>
        {#each planSteps as step}
          <li>{stepLabel(step)}</li>
        {/each}
      </ul>
    </details>
    <div class="btn-row btn-row-h">
      <button class="btn btn-secondary" onclick={() => screen = "profile"}>
        {text(locale, "generic_back")}
      </button>
      <button class="btn btn-primary" onclick={beginInstall}>
        {text(locale, "ready_cta")}
      </button>
    </div>
  </section>
{:else if screen === "installing"}
  <section class="card">
    <h1>{text(locale, "installing_lead")}</h1>
    <p class="lead">{text(locale, "installing_warning_dont_close")}</p>
    <div
      class="progress-track"
      role="progressbar"
      aria-valuenow={progressPercent}
      aria-valuemin="0"
      aria-valuemax="100"
    >
      <div
        class="progress-fill"
        class:indeterminate={progressIsIndeterminate}
        style={progressIsIndeterminate ? "" : `width: ${progressPercent}%`}
      ></div>
    </div>
    <div class="progress-label">
      {#if currentStep}
        {fill(text(locale, "installing_step_progress"), {
          current: currentStep.step_index + 1,
          total: currentStep.step_count,
        })}
        <div class="phase-text">{currentPhaseText}</div>
        <div class="current-step">{currentStepText}</div>
        {#if currentDownload && currentDownload.bytes_total > 0}
          <div class="download-detail">
            {formatBytes(currentDownload.bytes_downloaded)} of {formatBytes(currentDownload.bytes_total)}
          </div>
        {:else if isMac && currentStep.component_id === "main-app"}
          <div class="download-detail">
            macOS may show a small password prompt. That is normal.
          </div>
        {/if}
      {:else}
        {text(locale, "generic_loading")}
      {/if}
    </div>
  </section>
{:else if screen === "done"}
  <section class="card done">
    <div class="big-check">✓</div>
    <h1>{text(locale, "done_lead")}</h1>
    <p class="lead">{doneBody}</p>
    <div class="btn-row">
      <button class="btn btn-primary btn-block" onclick={closeWindow}>
        {text(locale, "done_cta_close")}
      </button>
    </div>
  </section>
{:else if screen === "error"}
  <section class="card">
    <div class="big-cross">×</div>
    <h1>{text(locale, "error_lead")}</h1>
    <p class="lead">{installError || text(locale, "error_body")}</p>
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
      <button class="btn btn-primary btn-block" onclick={beginInstall}>
        {text(locale, "error_cta_retry")}
      </button>
      <button class="btn btn-secondary btn-block" onclick={handleHelp} disabled={helpBusy}>
        {helpBusy ? text(locale, "generic_loading") : text(locale, "error_cta_help")}
      </button>
    </div>
  </section>
{/if}

<style>
  details {
    border: 2px solid var(--border);
    border-radius: 12px;
    padding: 16px 18px;
    background: var(--surface-2);
  }

  summary {
    cursor: pointer;
    font-weight: 600;
  }

  ul {
    margin: 14px 0 0;
    padding-left: 28px;
  }

  li {
    margin-bottom: 8px;
  }

  .done {
    text-align: center;
  }

  .current-step {
    margin-top: 8px;
    font-weight: 600;
  }

  .phase-text {
    margin-top: 10px;
    color: var(--primary);
    font-weight: 700;
  }

  .download-detail {
    margin-top: 4px;
    font-size: 0.9rem;
    color: var(--text-muted);
  }
</style>
