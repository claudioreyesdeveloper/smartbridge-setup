// Shapes returned by the Rust backend. Keep this file aligned with
// `src-tauri/src/wizard.rs`, `license.rs`, and the tiny identity commands
// still exposed from `commands.rs`.

export type SetupFlavor = "release" | "demo" | "beta_0_1";

export interface LicenseStatus {
  flavor: SetupFlavor;
  display_name: string;
  activated: boolean;
  email: string | null;
}

export interface ActivationOutcome {
  ok: boolean;
  message: string;
  status: LicenseStatus;
}

export interface UninstallMode {
  active: boolean;
  component: string | null;
}

export interface HostInfo {
  os: string;
  arch: string;
  family: string;
}

export interface ProfileChoice {
  use_ai_lyrics: boolean;
}

export interface PlanStep {
  component_id: string;
  label_en: string;
  label_de: string;
}

export interface InstallPlan {
  steps: PlanStep[];
}

export interface WizardStepEvent {
  step_index: number;
  step_count: number;
  component_id: string;
  status: "starting" | "ok" | "failed";
  failure_message: string | null;
}

export interface DownloadProgress {
  download_id: string;
  bytes_downloaded: number;
  bytes_total: number;
  phase:
    | "starting"
    | "downloading"
    | "verified"
    | "cache_hit"
    | "verifying_local"
    | "verified_local";
}

export interface InstallAllOutcome {
  success: boolean;
  failed_step_index: number | null;
  failed_component_id: string | null;
  failure_message: string;
  step_messages: string[][];
}

export interface UninstallAllOutcome {
  success: boolean;
  messages: string[];
}

export interface HelpEmailOutcome {
  bundle_path: string;
  mailto_url: string;
  help_email: string;
}
