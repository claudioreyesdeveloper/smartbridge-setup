<script lang="ts">
  import { onMount } from "svelte";
  import { getUninstallMode } from "$lib/api";
  import InstallWizard from "$lib/components/InstallWizard.svelte";
  import UninstallWizard from "$lib/components/UninstallWizard.svelte";
  import WizardShell from "$lib/components/WizardShell.svelte";
  import type { Locale } from "$lib/i18n/messages";
  import type { UninstallMode } from "$lib/types";
  import "$lib/styles/global.css";

  let locale = $state<Locale>("en");
  let highContrast = $state(false);
  let uninstallMode = $state<UninstallMode | null>(null);

  onMount(async () => {
    const savedLocale = localStorage.getItem("smartbridge-setup-locale");
    if (savedLocale === "en" || savedLocale === "de") {
      locale = savedLocale;
    } else if (navigator.language.toLowerCase().startsWith("de")) {
      locale = "de";
    }

    highContrast = localStorage.getItem("smartbridge-setup-high-contrast") === "1";
    applyContrast();

    try {
      uninstallMode = await getUninstallMode();
    } catch {
      uninstallMode = { active: false, component: null };
    }
  });

  function changeLocale(next: Locale) {
    locale = next;
    localStorage.setItem("smartbridge-setup-locale", next);
  }

  function toggleContrast() {
    highContrast = !highContrast;
    localStorage.setItem("smartbridge-setup-high-contrast", highContrast ? "1" : "0");
    applyContrast();
  }

  function applyContrast() {
    document.documentElement.classList.toggle("high-contrast", highContrast);
  }
</script>

<WizardShell
  {locale}
  {highContrast}
  onLocaleChange={changeLocale}
  onContrastToggle={toggleContrast}
>
  {#if uninstallMode === null}
    <section class="card">
      <h1>SmartBridge Setup</h1>
      <div class="progress-track" aria-hidden="true">
        <div class="progress-fill indeterminate"></div>
      </div>
    </section>
  {:else if uninstallMode.active}
    <UninstallWizard {locale} />
  {:else}
    <InstallWizard {locale} onLocaleChange={changeLocale} />
  {/if}
</WizardShell>
