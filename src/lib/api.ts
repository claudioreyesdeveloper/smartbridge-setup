import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  ActivationOutcome,
  HelpEmailOutcome,
  HostInfo,
  InstallAllOutcome,
  InstallPlan,
  LicenseStatus,
  ProfileChoice,
  UninstallAllOutcome,
  UninstallMode,
} from "$lib/types";

export function getUninstallMode() {
  return invoke<UninstallMode>("get_uninstall_mode");
}

export function getHostInfo() {
  return invoke<HostInfo>("host_info");
}

export function getLicenseStatus() {
  return invoke<LicenseStatus>("get_license_status");
}

export function activateBeta(email: string, serial: string) {
  return invoke<ActivationOutcome>("activate_beta", { email, serial });
}

export function getInstallPlan(profile: ProfileChoice) {
  return invoke<InstallPlan>("install_plan", { profile });
}

export function checkInternetConnection() {
  return invoke<void>("check_internet_connection");
}

export function installAll(profile: ProfileChoice) {
  return invoke<InstallAllOutcome>("install_all", { profile });
}

export function cancelInstall() {
  return invoke<void>("cancel_install");
}

export function undoSmartBridgeChanges() {
  return invoke<UninstallAllOutcome>("undo_smartbridge_changes");
}

export function uninstallAll(keepUserData: boolean) {
  return invoke<UninstallAllOutcome>("uninstall_all", {
    keepUserData,
  });
}

export async function getHelpByEmail() {
  const outcome = await invoke<HelpEmailOutcome>("compose_help_email");
  await openUrl(outcome.mailto_url);
  return outcome;
}
