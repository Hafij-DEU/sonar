import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import logo from "@/assets/sonar-logo.png";
import { useSettings } from "@/hooks/useSettings";
import { useOsType } from "@/hooks/useOsType";
import { formatKeyCombination } from "@/lib/utils/keyboard";
import { useModelStore } from "@/stores/modelStore";

export const BANGLA_MODEL = "bangla-whisper-medium-Q8_0";
export const ENGLISH_MODEL_DEFAULT =
  "handy-computer/parakeet-tdt-0.6b-v2-gguf/parakeet-tdt-0.6b-v2-Q8_0.gguf";

export const KEY_LINKS: Record<string, string> = {
  gemini: "https://aistudio.google.com/apikey",
  openrouter: "https://openrouter.ai/keys",
};

export const Mark: React.FC<{ size?: number }> = ({ size = 34 }) => (
  <img
    src={logo}
    width={size}
    height={size}
    alt=""
    className="sonar-mark"
    draggable={false}
  />
);

/** The keys of one binding, drawn as keycaps. */
export const ShortcutKeys: React.FC<{ id: string }> = ({ id }) => {
  const { getSetting } = useSettings();
  const os = useOsType();
  const binding = getSetting("bindings")?.[id]?.current_binding ?? "";
  const keys = formatKeyCombination(binding, os)
    .split("+")
    .map((k) => k.trim())
    .filter(Boolean);
  if (keys.length === 0) return <span className="sonar-muted">—</span>;
  return (
    <span className="sonar-keys">
      {keys.map((k, i) => (
        <kbd key={`${k}-${i}`}>{k}</kbd>
      ))}
    </span>
  );
};

export const useEnglishModelId = () => {
  const { getSetting } = useSettings();
  return getSetting("english_model") || ENGLISH_MODEL_DEFAULT;
};

export const useModelState = (id: string) => {
  const model = useModelStore((s) => s.models.find((m) => m.id === id));
  const downloading = useModelStore((s) => !!s.downloadingModels[id]);
  const verifying = useModelStore((s) => !!s.verifyingModels[id]);
  const percent = useModelStore((s) => s.downloadProgress[id]?.percentage);
  const download = useModelStore((s) => s.downloadModel);
  const cancel = useModelStore((s) => s.cancelDownload);
  const remove = useModelStore((s) => s.deleteModel);
  return {
    model,
    ready: !!model?.is_downloaded,
    downloading,
    verifying,
    percent: percent ?? 0,
    download: () => download(id),
    cancel: () => cancel(id),
    remove: () => remove(id),
  };
};

/** One offline model: its size and a download / progress / ready state. */
export const ModelLine: React.FC<{
  id: string;
  label: string;
  allowDelete?: boolean;
}> = ({ id, label, allowDelete = false }) => {
  const { t } = useTranslation();
  const s = useModelState(id);
  const size = s.model ? `${Math.round(s.model.size_mb)} MB` : "";
  return (
    <div className="sonar-model">
      <div className="sonar-model-text">
        <span className="sonar-model-name">{label}</span>
        <span className="sonar-muted">
          {s.model?.name ?? id}
          {size && ` · ${size}`}
        </span>
      </div>
      {s.ready ? (
        <span className="sonar-model-actions">
          <span className="sonar-tag sonar-tag-ok">
            {t("sonar.model.ready")}
          </span>
          {allowDelete && (
            <button
              type="button"
              className="sonar-btn sonar-btn-ghost"
              onClick={() => s.remove()}
            >
              {t("sonar.model.remove")}
            </button>
          )}
        </span>
      ) : s.downloading || s.verifying ? (
        <span className="sonar-model-actions">
          <span className="sonar-progress" aria-label={`${s.percent}%`}>
            <span style={{ width: `${Math.max(3, s.percent)}%` }} />
          </span>
          <span className="sonar-muted sonar-num">
            {s.verifying
              ? t("sonar.model.verifying")
              : `${Math.round(s.percent)}%`}
          </span>
          {s.downloading && (
            <button
              type="button"
              className="sonar-btn sonar-btn-ghost"
              onClick={() => s.cancel()}
            >
              {t("sonar.model.cancel")}
            </button>
          )}
        </span>
      ) : (
        <button
          type="button"
          className="sonar-btn sonar-btn-primary"
          onClick={() => s.download()}
          disabled={!s.model}
        >
          {t("sonar.model.download")}
        </button>
      )}
    </div>
  );
};

/** Offline / Cloud as a two-way switch. */
export const ModeSwitch: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const cloud = !!getSetting("cloud_transcription_enabled");
  const set = (value: boolean) => {
    if (value !== cloud) updateSetting("cloud_transcription_enabled", value);
  };
  return (
    <div className="sonar-seg" role="radiogroup">
      <button
        type="button"
        role="radio"
        aria-checked={!cloud}
        className={!cloud ? "on" : ""}
        onClick={() => set(false)}
      >
        <span className="sonar-seg-title">{t("sonar.mode.offline")}</span>
        <span className="sonar-seg-hint">{t("sonar.mode.offlineHint")}</span>
      </button>
      <button
        type="button"
        role="radio"
        aria-checked={cloud}
        className={cloud ? "on" : ""}
        onClick={() => set(true)}
      >
        <span className="sonar-seg-title">{t("sonar.mode.cloud")}</span>
        <span className="sonar-seg-hint">{t("sonar.mode.cloudHint")}</span>
      </button>
    </div>
  );
};

export const useCloudKey = () => {
  const { getSetting } = useSettings();
  const provider = getSetting("cloud_transcription_provider_id") || "gemini";
  const key = getSetting("post_process_api_keys")?.[provider] ?? "";
  return { provider, key, hasKey: key.trim().length > 0 };
};

/** The cloud provider's API key, saved on Save or Enter. */
export const KeyField: React.FC = () => {
  const { t } = useTranslation();
  const { updatePostProcessApiKey } = useSettings();
  const { provider, key, hasKey } = useCloudKey();
  const [draft, setDraft] = useState(key);
  const [saved, setSaved] = useState(false);
  useEffect(() => setDraft(key), [key]);

  const save = async () => {
    await updatePostProcessApiKey(provider, draft.trim());
    setSaved(true);
    window.setTimeout(() => setSaved(false), 1600);
  };

  return (
    <div className="sonar-keyfield">
      <label className="sonar-label" htmlFor="sonar-key">
        {t("sonar.cloud.keyLabel", {
          provider: t(`sonar.cloud.provider.${provider}`),
        })}
      </label>
      <div className="sonar-keyrow">
        <input
          id="sonar-key"
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={draft}
          placeholder={t("sonar.cloud.keyPlaceholder")}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && save()}
        />
        <button
          type="button"
          className="sonar-btn sonar-btn-primary"
          onClick={save}
          disabled={draft.trim() === key.trim()}
        >
          {saved ? t("sonar.cloud.saved") : t("sonar.cloud.save")}
        </button>
      </div>
      <div className="sonar-keyhelp">
        <span className={hasKey ? "sonar-ok" : "sonar-warn"}>
          {hasKey ? t("sonar.cloud.keySet") : t("sonar.cloud.keyMissing")}
        </span>
        {KEY_LINKS[provider] && (
          <button
            type="button"
            className="sonar-link"
            onClick={() => openUrl(KEY_LINKS[provider])}
          >
            {t("sonar.cloud.getKey")}
          </button>
        )}
      </div>
    </div>
  );
};

export const relativeTime = (
  timestampSecs: number,
  lang: string,
): string => {
  const diff = Date.now() / 1000 - timestampSecs;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  if (diff < 60) return rtf.format(-Math.round(diff), "second");
  if (diff < 3600) return rtf.format(-Math.round(diff / 60), "minute");
  if (diff < 86400) return rtf.format(-Math.round(diff / 3600), "hour");
  return new Date(timestampSecs * 1000).toLocaleDateString(lang);
};
