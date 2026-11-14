import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import {
  BANGLA_MODEL,
  KeyField,
  Mark,
  ModeSwitch,
  ModelLine,
  ShortcutKeys,
  useEnglishModelId,
} from "./parts";

/** First run: pick offline or cloud, fetch a model or add a key, try it. */
export const SetupView: React.FC<{ onComplete: () => void }> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [step, setStep] = useState(0);
  const cloud = !!getSetting("cloud_transcription_enabled");
  const englishId = useEnglishModelId();

  useEffect(() => {
    if (step === 2) {
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => console.warn("Failed to initialize:", e));
    }
  }, [step]);

  const finish = async () => {
    try {
      await commands.completeOnboarding();
    } catch (e) {
      console.warn("completeOnboarding failed:", e);
    }
    onComplete();
  };

  return (
    <div className="sonar-root sonar-setup">
      <div className="sonar-setup-card">
        <ol className="sonar-steps" aria-label={t("sonar.setup.progress")}>
          {[0, 1, 2].map((i) => (
            <li key={i} className={i <= step ? "on" : ""} />
          ))}
        </ol>

        {step === 0 && (
          <div className="sonar-setup-body center">
            <Mark size={96} />
            <h1>{t("sonar.setup.welcomeTitle")}</h1>
            <p className="sonar-muted">{t("sonar.setup.welcomeBody")}</p>
            <button
              type="button"
              className="sonar-btn sonar-btn-primary sonar-btn-lg"
              onClick={() => setStep(1)}
            >
              {t("sonar.setup.start")}
            </button>
          </div>
        )}

        {step === 1 && (
          <div className="sonar-setup-body">
            <h1>{t("sonar.setup.modeTitle")}</h1>
            <p className="sonar-muted">{t("sonar.setup.modeBody")}</p>
            <ModeSwitch />
            {cloud ? (
              <KeyField />
            ) : (
              <div className="sonar-setup-models">
                <ModelLine id={BANGLA_MODEL} label={t("sonar.lang.bangla")} />
                <ModelLine id={englishId} label={t("sonar.lang.english")} />
                <p className="sonar-muted sonar-small">
                  {t("sonar.setup.downloadNote")}
                </p>
              </div>
            )}
            <div className="sonar-setup-nav">
              <button
                type="button"
                className="sonar-btn sonar-btn-quiet"
                onClick={() => setStep(0)}
              >
                {t("sonar.setup.back")}
              </button>
              <button
                type="button"
                className="sonar-btn sonar-btn-primary"
                onClick={() => setStep(2)}
              >
                {t("sonar.setup.next")}
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="sonar-setup-body">
            <h1>{t("sonar.setup.tryTitle")}</h1>
            <p className="sonar-muted">{t("sonar.setup.tryBody")}</p>
            <div className="sonar-try-keys">
              <div>
                <span className="sonar-label">{t("sonar.lang.bangla")}</span>
                <ShortcutKeys id="transcribe" />
              </div>
              <div>
                <span className="sonar-label">{t("sonar.lang.english")}</span>
                <ShortcutKeys id="transcribe_english" />
              </div>
            </div>
            <textarea
              className="sonar-try"
              placeholder={t("sonar.setup.tryPlaceholder")}
              rows={4}
              autoFocus
            />
            <div className="sonar-setup-nav">
              <button
                type="button"
                className="sonar-btn sonar-btn-quiet"
                onClick={() => setStep(1)}
              >
                {t("sonar.setup.back")}
              </button>
              <button
                type="button"
                className="sonar-btn sonar-btn-primary"
                onClick={finish}
              >
                {t("sonar.setup.finish")}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
