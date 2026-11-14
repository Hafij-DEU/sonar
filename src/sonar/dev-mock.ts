// Browser-only preview of the UI with a fake backend (dev aid, not shipped).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const q = new URLSearchParams(location.search);
(window as any).__TAURI_OS_PLUGIN_INTERNALS__ = {
  platform: "linux", os_type: "linux", family: "unix", version: "1",
  arch: "x86_64", eol: "\n", exe_extension: "",
};
mockWindows("main");

const binding = (id: string, name: string, keys: string) => ({
  id, name, description: "", default_binding: keys, current_binding: keys,
});
const settings: any = {
  onboarding_completed: q.get("setup") !== "1",
  bindings: {
    transcribe: binding("transcribe", "Transcribe Bangla", "super+alt+space"),
    transcribe_english: binding("transcribe_english", "Transcribe English", "super+alt+shift+space"),
    cancel: binding("cancel", "Cancel", "escape"),
  },
  cloud_transcription_enabled: q.get("cloud") === "1",
  cloud_transcription_provider_id: "gemini",
  cloud_transcription_quality: "fast",
  english_model: "handy-computer/parakeet-tdt-0.6b-v2-gguf/parakeet-tdt-0.6b-v2-Q8_0.gguf",
  post_process_api_keys: { gemini: q.get("key") === "1" ? "AIza-test" : "" },
  post_process_providers: [], post_process_models: {}, post_process_prompts: [],
  selected_microphone: "default", theme: q.get("theme") ?? "system",
  app_language: q.get("lang") ?? "en", audio_feedback: true,
  history_limit: 50, spoken_languages: [], selected_language: "bn",
  show_whats_new_on_update: false, whats_new_last_seen_version: "99.0.0",
};
const model = (id: string, name: string, size: number, done: boolean) => ({
  id, name, description: "", filename: id, source: "Catalog", size_mb: size,
  is_downloaded: done, is_downloading: false, partial_size: 0,
  accuracy_score: 0.8, speed_score: 0.6, supports_translation: false,
  is_recommended: true, supported_languages: [], supports_language_selection: false,
  is_custom: false, supports_streaming: false, supports_language_detection: false,
});
const models = [
  model("bangla-whisper-medium-Q8_0", "Bangla Whisper Medium", 793, q.get("models") === "1"),
  model(settings.english_model, "Parakeet TDT 0.6B v2", 640, q.get("models") === "1"),
];
const now = Date.now() / 1000;
const entry = (id: number, text: string, ago: number) => ({
  id, file_name: "", timestamp: now - ago, saved: false, title: "",
  transcription_text: text, post_processed_text: null, post_process_prompt: null,
  post_process_requested: false,
});
const history = q.get("empty") === "1" ? [] : [
  entry(5, "আজকে বিকেলে মিটিংটা চারটায় হবে, সবাইকে জানিয়ে দিও।", 40),
  entry(4, "Please send the revised budget before Friday.", 600),
  entry(3, "ধন্যবাদ, ফাইলটা পেয়েছি। আমি রাতে দেখে জানাব।", 5400),
  entry(2, "Remind me to call the bank tomorrow morning.", 90000),
  entry(1, "", 190000),
];

mockIPC((cmd, args: any) => {
  switch (cmd) {
    case "get_app_settings":
    case "get_default_settings":
      return settings;
    case "get_available_models":
      return models;
    case "get_current_model":
      return models[0].id;
    case "get_history_entries":
      return { entries: history.slice(0, args?.limit ?? 30), has_more: false };
    case "get_available_microphones":
    case "get_available_output_devices":
      return [{ index: "default", name: "Default", is_default: true }];
    case "change_cloud_transcription_enabled_setting":
      settings.cloud_transcription_enabled = args.enabled; return null;
    case "get_retired_models":
    case "get_active_notices":
    case "get_transcription_jobs":
      return [];
    case "plugin:event|listen":
      return Math.floor(Math.random() * 1e6);
    default:
      return null;
  }
});

await import("../main");
