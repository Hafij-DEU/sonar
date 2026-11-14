import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ShortcutInput } from "@/components/settings/ShortcutInput";
import { ShortcutActivationSetting } from "@/components/settings/ShortcutActivation";
import { MicrophoneSelector } from "@/components/settings/MicrophoneSelector";
import { AudioFeedback } from "@/components/settings/AudioFeedback";
import { MuteWhileRecording } from "@/components/settings/MuteWhileRecording";
import { AppLanguageSelector } from "@/components/settings/AppLanguageSelector";
import { ThemeSelector } from "@/components/settings/ThemeSelector";
import { AutostartToggle } from "@/components/settings/AutostartToggle";
import { StartHidden } from "@/components/settings/StartHidden";
import { ShowOverlay } from "@/components/settings/ShowOverlay";
import { PasteMethodSetting } from "@/components/settings/PasteMethod";
import { ModelsSettings } from "@/components/settings";
import { AdvancedSettings } from "@/components/settings/advanced/AdvancedSettings";
import { AboutSettings } from "@/components/settings/about/AboutSettings";
import {
  BANGLA_MODEL,
  KeyField,
  ModeSwitch,
  ModelLine,
  useEnglishModelId,
} from "./parts";

type Sub = null | "models" | "advanced" | "about";

const Row: React.FC<{
  title: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ title, hint, children }) => (
  <div className="sonar-row">
    <div>
      <div className="sonar-row-title">{title}</div>
      {hint && <div className="sonar-muted">{hint}</div>}
    </div>
    <div className="sonar-row-control">{children}</div>
  </div>
);

const Choice: React.FC<{
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}> = ({ value, options, onChange }) => (
  <div className="sonar-chips" role="radiogroup">
    {options.map((o) => (
      <button
        key={o.value}
        type="button"
        role="radio"
        aria-checked={value === o.value}
        className={value === o.value ? "on" : ""}
        onClick={() => onChange(o.value)}
      >
        {o.label}
      </button>
    ))}
  </div>
);

export const SettingsView: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const [sub, setSub] = useState<Sub>(null);
  const englishId = useEnglishModelId();
  const cloud = !!getSetting("cloud_transcription_enabled");
  const provider = getSetting("cloud_transcription_provider_id") || "gemini";
  const quality = getSetting("cloud_transcription_quality") ?? "fast";

  if (sub) {
    return (
      <div className="sonar-page sonar-sub">
        <button
          type="button"
          className="sonar-link sonar-back"
          onClick={() => setSub(null)}
        >
          ← {t("sonar.settings.back")}
        </button>
        <div
          className={`sonar-legacy ${sub === "models" ? "sonar-legacy-fill" : ""}`}
        >
          {sub === "models" && <ModelsSettings onChangeLanguages={() => {}} />}
          {sub === "advanced" && <AdvancedSettings />}
          {sub === "about" && <AboutSettings />}
        </div>
      </div>
    );
  }

  return (
    <div className="sonar-page sonar-settings">
      <SettingsGroup title={t("sonar.settings.shortcuts")}>
        <ShortcutInput shortcutId="transcribe" grouped />
        <ShortcutInput shortcutId="transcribe_english" grouped />
        <ShortcutActivationSetting descriptionMode="tooltip" grouped />
      </SettingsGroup>

      <section className="sonar-card">
        <h2 className="sonar-h2">{t("sonar.mode.title")}</h2>
        <ModeSwitch />
        {cloud && (
          <>
            <Row title={t("sonar.cloud.providerTitle")}>
              <Choice
                value={provider}
                onChange={(v) =>
                  updateSetting("cloud_transcription_provider_id", v)
                }
                options={[
                  { value: "gemini", label: t("sonar.cloud.provider.gemini") },
                  {
                    value: "openrouter",
                    label: t("sonar.cloud.provider.openrouter"),
                  },
                ]}
              />
            </Row>
            <Row
              title={t("sonar.cloud.qualityTitle")}
              hint={t(`sonar.cloud.qualityHint.${quality}`)}
            >
              <Choice
                value={quality}
                onChange={(v) =>
                  updateSetting(
                    "cloud_transcription_quality",
                    v as "fast" | "accurate",
                  )
                }
                options={[
                  { value: "fast", label: t("sonar.cloud.quality.fast") },
                  {
                    value: "accurate",
                    label: t("sonar.cloud.quality.accurate"),
                  },
                ]}
              />
            </Row>
            <KeyField />
            <p className="sonar-muted sonar-small">
              {t("sonar.cloud.privacy")}
            </p>
          </>
        )}
      </section>

      <section className="sonar-card">
        <h2 className="sonar-h2">{t("sonar.settings.models")}</h2>
        <ModelLine
          id={BANGLA_MODEL}
          label={t("sonar.lang.bangla")}
          allowDelete
        />
        <ModelLine id={englishId} label={t("sonar.lang.english")} allowDelete />
        <button
          type="button"
          className="sonar-link"
          onClick={() => setSub("models")}
        >
          {t("sonar.settings.allModels")}
        </button>
      </section>

      <SettingsGroup title={t("sonar.settings.sound")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped />
        <MuteWhileRecording descriptionMode="tooltip" grouped />
        <AudioFeedback descriptionMode="tooltip" grouped />
      </SettingsGroup>

      <SettingsGroup title={t("sonar.settings.app")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped />
        <ThemeSelector descriptionMode="tooltip" grouped />
        <AutostartToggle descriptionMode="tooltip" grouped />
        <StartHidden descriptionMode="tooltip" grouped />
        <ShowOverlay descriptionMode="tooltip" grouped />
        <PasteMethodSetting descriptionMode="tooltip" grouped />
      </SettingsGroup>

      <div className="sonar-subnav">
        <button type="button" onClick={() => setSub("advanced")}>
          {t("sonar.settings.advanced")}
        </button>
        <button type="button" onClick={() => setSub("about")}>
          {t("sonar.settings.about")}
        </button>
      </div>
    </div>
  );
};
