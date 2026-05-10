// SmartBridge Setup - i18n message bag.
//
// Keep keys flat and human-friendly. Every visible string in the wizard
// MUST live here (or be a Rust-supplied label like the install plan
// step labels which are translated server-side). Do NOT hard-code
// English in .svelte files.
//
// New languages: copy `en` to a new top-level key and translate. The
// `Locale` type ensures a missing key in any locale becomes a TS error.
//
// Style:
//   * Short, plain sentences. No jargon, no acronyms.
//   * "We" / "you" voice. The installer talks like a person.
//   * No exclamation marks except on the success screen.

export type Locale = "en" | "de";

export interface Messages {
  app_title: string;

  // Welcome
  welcome_lead: string;
  welcome_body: string;
  welcome_cta_start: string;
  welcome_language_label: string;
  welcome_high_contrast: string;

  // Activate (beta)
  activate_lead: string;
  activate_body: string;
  activate_email_label: string;
  activate_serial_label: string;
  activate_cta: string;
  activate_quit: string;
  activate_invalid: string;

  // Profile
  profile_lead: string;
  profile_body: string;
  profile_q_cubase: string;
  profile_q_cubase_sub: string;
  profile_q_synthv: string;
  profile_q_synthv_sub: string;
  profile_q_ai: string;
  profile_q_ai_sub: string;
  profile_long_warning: string;
  profile_yes: string;
  profile_no: string;
  profile_cta: string;

  // Ready
  ready_lead: string;
  ready_body_one: string;
  ready_body_many: string;
  ready_long_warning: string;
  ready_what: string;
  ready_cta: string;

  // Internet
  internet_lead: string;
  internet_body: string;
  internet_cta_retry: string;

  // Installing
  installing_lead: string;
  installing_step_progress: string;
  installing_warning_dont_close: string;
  download_detail: string;
  download_time_estimating: string;
  download_time_remaining: string;
  time_less_than_minute: string;
  time_one_minute: string;
  time_minutes: string;
  time_one_hour: string;
  time_hours: string;

  // Done
  done_lead: string;
  done_body_windows: string;
  done_body_macos: string;
  done_cta_close: string;

  // Error
  error_lead: string;
  error_body: string;
  error_cta_retry: string;
  error_cta_help: string;
  error_help_lead: string;
  error_help_body: string;
  error_help_address_intro: string;
  error_close: string;

  // Generic
  generic_back: string;
  generic_cancel: string;
  generic_quit: string;
  generic_loading: string;

  // Uninstall
  uninstall_confirm_lead: string;
  uninstall_confirm_body: string;
  uninstall_confirm_cta: string;
  uninstall_keep_lead: string;
  uninstall_keep_q: string;
  uninstall_keep_yes: string;
  uninstall_keep_yes_sub: string;
  uninstall_keep_no: string;
  uninstall_keep_no_sub: string;
  uninstall_progress_lead: string;
  uninstall_progress_body: string;
  uninstall_done_lead: string;
  uninstall_done_body: string;
  uninstall_failed_lead: string;
  uninstall_failed_body: string;
}

export const MESSAGES: Record<Locale, Messages> = {
  en: {
    app_title: "SmartBridge Setup",

    welcome_lead: "Welcome.",
    welcome_body:
      "This will set up SmartBridge on your computer. It takes about five minutes. We will walk you through it step by step.",
    welcome_cta_start: "Get started",
    welcome_language_label: "Language",
    welcome_high_contrast: "Bigger contrast",

    activate_lead: "Enter the code from your email.",
    activate_body:
      "Please type in the email address and the code that we sent you. Both can be in upper or lower case.",
    activate_email_label: "Your email address",
    activate_serial_label: "Your code",
    activate_cta: "Continue",
    activate_quit: "Quit",
    activate_invalid:
      "That code does not match this email. Please check both and try again.",

    profile_lead: "One optional extra.",
    profile_body:
      "SmartBridge will set up the normal music connections automatically. Please choose whether you also want AI lyrics.",
    profile_q_cubase: "Do you use Cubase?",
    profile_q_cubase_sub:
      "Cubase is a music production program by Steinberg.",
    profile_q_synthv: "Do you use Synthesizer V?",
    profile_q_synthv_sub:
      "Synthesizer V is a singing-voice program by Dreamtonics.",
    profile_q_ai: "Do you want help writing lyrics?",
    profile_q_ai_sub:
      "Adds a smart helper that suggests lyric ideas. Needs a slow one-time download of about 10 GB.",
    profile_long_warning:
      "You chose the large optional download. This can take a long time, especially on slower internet.",
    profile_yes: "Yes",
    profile_no: "No, thank you",
    profile_cta: "Continue",

    ready_lead: "Ready to install.",
    ready_body_one: "We will install one thing on your computer.",
    ready_body_many: "We will install {count} things on your computer.",
    ready_long_warning:
      "This setup includes AI lyrics. The first download is about 10 GB, so please leave the computer on and connected to the internet.",
    ready_what: "Show me what",
    ready_cta: "Install now",

    internet_lead: "Please connect to the internet.",
    internet_body:
      "SmartBridge Setup needs the internet to download the installer files. Please check Wi-Fi or the network cable, then try again.",
    internet_cta_retry: "Check again",

    installing_lead: "Installing now.",
    installing_step_progress: "Step {current} of {total}",
    installing_warning_dont_close:
      "Please leave this window open. It will tell you when it is done.",
    download_detail: "{downloaded} of {total}",
    download_time_estimating: "Estimating time remaining...",
    download_time_remaining: "About {time} remaining",
    time_less_than_minute: "less than a minute",
    time_one_minute: "1 minute",
    time_minutes: "{count} minutes",
    time_one_hour: "1 hour",
    time_hours: "{count} hours",

    done_lead: "All done.",
    done_body_windows:
      "SmartBridge is ready. You can find it in your Start menu under SmartBridge.",
    done_body_macos:
      "SmartBridge is ready. You can find it in your Applications folder. If macOS asks the first time, choose Open.",
    done_cta_close: "Close",

    error_lead: "Something did not work.",
    error_body:
      "Do not worry - nothing on your computer is broken. You can try again, or send us a help report and we will look at it for you.",
    error_cta_retry: "Try again",
    error_cta_help: "Get help by email",
    error_help_lead: "We saved a help file to your Desktop.",
    error_help_body:
      "We have opened your email program. Before you click Send, please drag the help file from your Desktop into the email.",
    error_help_address_intro: "If the email program did not open, please write to",
    error_close: "Close",

    generic_back: "Back",
    generic_cancel: "Cancel",
    generic_quit: "Quit",
    generic_loading: "Loading...",

    uninstall_confirm_lead: "Remove SmartBridge?",
    uninstall_confirm_body:
      "This will remove SmartBridge from this computer. You can choose whether to keep your songs and settings.",
    uninstall_confirm_cta: "Remove SmartBridge",
    uninstall_keep_lead: "Keep your songs and settings?",
    uninstall_keep_q: "What should we do with your personal SmartBridge files?",
    uninstall_keep_yes: "Keep them",
    uninstall_keep_yes_sub:
      "Best choice if you might use SmartBridge again later.",
    uninstall_keep_no: "Delete them",
    uninstall_keep_no_sub:
      "Only choose this if you are sure you do not need them any more.",
    uninstall_progress_lead: "Removing SmartBridge.",
    uninstall_progress_body:
      "Please leave this window open. This should only take a minute.",
    uninstall_done_lead: "SmartBridge has been removed.",
    uninstall_done_body: "You can close this window now.",
    uninstall_failed_lead: "We could not remove everything.",
    uninstall_failed_body:
      "Nothing else on your computer is broken. Please use Get help by email and we will look at it for you.",
  },

  de: {
    app_title: "SmartBridge Einrichtung",

    welcome_lead: "Willkommen.",
    welcome_body:
      "Dieses Programm richtet SmartBridge auf Ihrem Computer ein. Es dauert ungef\u00e4hr f\u00fcnf Minuten. Wir gehen Schritt f\u00fcr Schritt vor.",
    welcome_cta_start: "Loslegen",
    welcome_language_label: "Sprache",
    welcome_high_contrast: "Mehr Kontrast",

    activate_lead: "Geben Sie den Code aus Ihrer E-Mail ein.",
    activate_body:
      "Bitte geben Sie Ihre E-Mail-Adresse und den Code ein, den wir Ihnen geschickt haben. Gro\u00df- und Kleinschreibung ist egal.",
    activate_email_label: "Ihre E-Mail-Adresse",
    activate_serial_label: "Ihr Code",
    activate_cta: "Weiter",
    activate_quit: "Beenden",
    activate_invalid:
      "Dieser Code passt nicht zu dieser E-Mail-Adresse. Bitte pr\u00fcfen Sie beides und versuchen Sie es erneut.",

    profile_lead: "Eine optionale Zusatzfunktion.",
    profile_body:
      "SmartBridge richtet die normalen Musikverbindungen automatisch ein. Bitte wählen Sie nur aus, ob Sie auch KI-Liedtexte möchten.",
    profile_q_cubase: "Benutzen Sie Cubase?",
    profile_q_cubase_sub:
      "Cubase ist ein Musikprogramm von Steinberg.",
    profile_q_synthv: "Benutzen Sie Synthesizer V?",
    profile_q_synthv_sub:
      "Synthesizer V ist ein Gesangsprogramm von Dreamtonics.",
    profile_q_ai: "M\u00f6chten Sie Hilfe beim Schreiben von Liedtexten?",
    profile_q_ai_sub:
      "F\u00fcgt einen Helfer hinzu, der Textideen vorschl\u00e4gt. Der erste Download ist langsam und etwa 10 GB gro\u00df.",
    profile_long_warning:
      "Sie haben den gro\u00dfen optionalen Download gew\u00e4hlt. Das kann lange dauern, besonders bei langsamem Internet.",
    profile_yes: "Ja",
    profile_no: "Nein, danke",
    profile_cta: "Weiter",

    ready_lead: "Bereit zum Installieren.",
    ready_body_one: "Wir installieren eine Sache auf Ihrem Computer.",
    ready_body_many: "Wir installieren {count} Dinge auf Ihrem Computer.",
    ready_long_warning:
      "Diese Einrichtung enth\u00e4lt KI-Liedtexte. Der erste Download ist etwa 10 GB gro\u00df. Bitte lassen Sie den Computer eingeschaltet und mit dem Internet verbunden.",
    ready_what: "Was genau?",
    ready_cta: "Jetzt installieren",

    internet_lead: "Bitte mit dem Internet verbinden.",
    internet_body:
      "SmartBridge Einrichtung braucht das Internet, um die Installationsdateien herunterzuladen. Bitte pr\u00fcfen Sie WLAN oder Netzwerkkabel und versuchen Sie es erneut.",
    internet_cta_retry: "Erneut pr\u00fcfen",

    installing_lead: "Installation l\u00e4uft.",
    installing_step_progress: "Schritt {current} von {total}",
    installing_warning_dont_close:
      "Bitte lassen Sie dieses Fenster offen. Wir sagen Ihnen, wenn alles fertig ist.",
    download_detail: "{downloaded} von {total}",
    download_time_estimating: "Restzeit wird geschätzt...",
    download_time_remaining: "Noch ungefähr {time}",
    time_less_than_minute: "weniger als 1 Minute",
    time_one_minute: "1 Minute",
    time_minutes: "{count} Minuten",
    time_one_hour: "1 Stunde",
    time_hours: "{count} Stunden",

    done_lead: "Fertig.",
    done_body_windows:
      "SmartBridge ist bereit. Sie finden es im Startmen\u00fc unter SmartBridge.",
    done_body_macos:
      "SmartBridge ist bereit. Sie finden es im Programme-Ordner. Wenn macOS beim ersten Mal fragt, w\u00e4hlen Sie \u00d6ffnen.",
    done_cta_close: "Schlie\u00dfen",

    error_lead: "Etwas hat nicht funktioniert.",
    error_body:
      "Keine Sorge - auf Ihrem Computer ist nichts kaputt. Sie k\u00f6nnen es erneut versuchen oder uns einen Hilfsbericht per E-Mail senden.",
    error_cta_retry: "Erneut versuchen",
    error_cta_help: "Hilfe per E-Mail",
    error_help_lead: "Wir haben eine Hilfsdatei auf Ihrem Schreibtisch gespeichert.",
    error_help_body:
      "Wir haben Ihr E-Mail-Programm ge\u00f6ffnet. Bevor Sie auf Senden klicken, ziehen Sie bitte die Hilfsdatei von Ihrem Schreibtisch in die E-Mail.",
    error_help_address_intro:
      "Wenn das E-Mail-Programm nicht ge\u00f6ffnet wurde, schreiben Sie bitte an",
    error_close: "Schlie\u00dfen",

    generic_back: "Zur\u00fcck",
    generic_cancel: "Abbrechen",
    generic_quit: "Beenden",
    generic_loading: "Wird geladen...",

    uninstall_confirm_lead: "SmartBridge entfernen?",
    uninstall_confirm_body:
      "Damit wird SmartBridge von diesem Computer entfernt. Sie k\u00f6nnen ausw\u00e4hlen, ob Ihre Lieder und Einstellungen bleiben sollen.",
    uninstall_confirm_cta: "SmartBridge entfernen",
    uninstall_keep_lead: "Lieder und Einstellungen behalten?",
    uninstall_keep_q:
      "Was sollen wir mit Ihren pers\u00f6nlichen SmartBridge-Dateien tun?",
    uninstall_keep_yes: "Behalten",
    uninstall_keep_yes_sub:
      "Die beste Wahl, wenn Sie SmartBridge vielleicht sp\u00e4ter wieder benutzen.",
    uninstall_keep_no: "L\u00f6schen",
    uninstall_keep_no_sub:
      "Nur ausw\u00e4hlen, wenn Sie sicher sind, dass Sie sie nicht mehr brauchen.",
    uninstall_progress_lead: "SmartBridge wird entfernt.",
    uninstall_progress_body:
      "Bitte lassen Sie dieses Fenster offen. Das sollte nur eine Minute dauern.",
    uninstall_done_lead: "SmartBridge wurde entfernt.",
    uninstall_done_body: "Sie k\u00f6nnen dieses Fenster jetzt schlie\u00dfen.",
    uninstall_failed_lead: "Wir konnten nicht alles entfernen.",
    uninstall_failed_body:
      "Auf Ihrem Computer ist nichts anderes kaputt. Bitte nutzen Sie Hilfe per E-Mail, dann schauen wir es uns an.",
  },
};

export function text(locale: Locale, key: keyof Messages): string {
  return MESSAGES[locale][key];
}

export function fill(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    String(values[key] ?? "")
  );
}