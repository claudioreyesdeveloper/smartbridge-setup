<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Locale } from "$lib/i18n/messages";
  import { text } from "$lib/i18n/messages";

  interface Props {
    locale: Locale;
    highContrast: boolean;
    children: Snippet;
    showLanguage?: boolean;
    onLocaleChange?: (locale: Locale) => void;
    onContrastToggle?: () => void;
  }

  let {
    locale,
    highContrast,
    children,
    showLanguage = true,
    onLocaleChange,
    onContrastToggle,
  }: Props = $props();
</script>

<div class="shell">
  <header class="shell-header">
    <div class="shell-title">{text(locale, "app_title")}</div>
    <div class="shell-toolbar">
      {#if showLanguage}
        <label class="language">
          <span>{text(locale, "welcome_language_label")}</span>
          <select
            value={locale}
            onchange={(e) =>
              onLocaleChange?.((e.currentTarget as HTMLSelectElement).value as Locale)}
          >
            <option value="en">English</option>
            <option value="de">Deutsch</option>
          </select>
        </label>
      {/if}
      <button class="contrast" type="button" onclick={() => onContrastToggle?.()}>
        {text(locale, "welcome_high_contrast")}
        {highContrast ? " on" : ""}
      </button>
    </div>
  </header>

  <main class="shell-main">
    {@render children()}
  </main>
</div>

<style>
  .language {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  select {
    font: inherit;
    font-size: 0.85rem;
    min-height: 44px;
    border-radius: 8px;
    border: 2px solid var(--border);
    background: var(--surface);
    color: var(--text);
    padding: 4px 10px;
  }

  .contrast {
    min-height: 44px;
    border-radius: 8px;
    border: 2px solid var(--border);
    background: var(--surface);
    color: var(--text);
    padding: 4px 12px;
    font-size: 0.85rem;
  }
</style>
