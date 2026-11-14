import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import {
  BANGLA_MODEL,
  KeyField,
  ModeSwitch,
  ShortcutEditor,
  useCloudKey,
  useEnglishModelId,
  useModelState,
} from "./parts";
import { HistoryList } from "./HistoryList";

type Status = "ready" | "needModel" | "needKey" | "downloading";

const useStatus = (): Status => {
  const { getSetting } = useSettings();
  const cloud = !!getSetting("cloud_transcription_enabled");
  const bangla = useModelState(BANGLA_MODEL);
  const english = useModelState(useEnglishModelId());
  const { hasKey } = useCloudKey();
  if (cloud) return hasKey ? "ready" : "needKey";
  if (bangla.ready || english.ready) return "ready";
  if (bangla.downloading || english.downloading) return "downloading";
  return "needModel";
};

/** The radar disc: sweeps while ready, dims while something is missing. */
const Scope: React.FC<{ status: Status }> = ({ status }) => (
  <div className={`sonar-scope ${status}`} aria-hidden="true">
    <span className="ring r1" />
    <span className="ring r2" />
    <span className="ring r3" />
    <span className="sweep" />
    <span className="blip b1" />
    <span className="blip b2" />
    <span className="core" />
  </div>
);

const Channel: React.FC<{
  bindingId: string;
  script: string;
  name: string;
  modelId: string;
}> = ({ bindingId, script, name, modelId }) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const cloud = !!getSetting("cloud_transcription_enabled");
  const quality = getSetting("cloud_transcription_quality") ?? "fast";
  const provider = getSetting("cloud_transcription_provider_id") || "gemini";
  const m = useModelState(modelId);

  let engine: React.ReactNode;
  if (cloud) {
    engine = (
      <span className="sonar-tag sonar-tag-cloud">
        {t(`sonar.cloud.provider.${provider}`)} ·{" "}
        {t(`sonar.cloud.quality.${quality}`)}
      </span>
    );
  } else if (m.ready) {
    engine = (
      <span className="sonar-tag sonar-tag-ok">{t("sonar.model.ready")}</span>
    );
  } else if (m.downloading || m.verifying) {
    engine = (
      <span className="sonar-tag">
        {t("sonar.model.downloadingShort", {
          percent: Math.round(m.percent),
        })}
      </span>
    );
  } else {
    engine = (
      <button
        type="button"
        className="sonar-btn sonar-btn-primary sonar-btn-sm"
        onClick={() => m.download()}
        disabled={!m.model}
      >
        {t("sonar.model.downloadSize", {
          size: m.model ? Math.round(m.model.size_mb) : "—",
        })}
      </button>
    );
  }

  return (
    <section className="sonar-channel">
      <div className="sonar-channel-head">
        <span className="sonar-script" lang={script === "অ" ? "bn" : "en"}>
          {script}
        </span>
        <div>
          <h3>{name}</h3>
          <p className="sonar-muted">{t("sonar.home.holdOrTap")}</p>
        </div>
      </div>
      <ShortcutEditor id={bindingId} />
      <div className="sonar-channel-foot">{engine}</div>
    </section>
  );
};

export const HomeView: React.FC<{ onOpenHistory: () => void }> = ({
  onOpenHistory,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const status = useStatus();
  const cloud = !!getSetting("cloud_transcription_enabled");
  const englishId = useEnglishModelId();

  return (
    <div className="sonar-page">
      <section className="sonar-hero">
        <Scope status={status} />
        <div className="sonar-hero-text">
          <p className="sonar-eyebrow">
            {cloud ? t("sonar.mode.cloud") : t("sonar.mode.offline")}
          </p>
          <h1>{t(`sonar.status.${status}.title`)}</h1>
          <p className="sonar-muted">{t(`sonar.status.${status}.body`)}</p>
        </div>
      </section>

      <div className="sonar-channels">
        <Channel
          bindingId="transcribe"
          script="অ"
          name={t("sonar.lang.bangla")}
          modelId={BANGLA_MODEL}
        />
        <Channel
          bindingId="transcribe_english"
          script="A"
          name={t("sonar.lang.english")}
          modelId={englishId}
        />
      </div>

      <section className="sonar-card">
        <h2 className="sonar-h2">{t("sonar.mode.title")}</h2>
        <ModeSwitch />
        {cloud && <KeyField />}
      </section>

      <section className="sonar-card">
        <div className="sonar-card-head">
          <h2 className="sonar-h2">{t("sonar.home.recent")}</h2>
          <button
            type="button"
            className="sonar-link"
            onClick={onOpenHistory}
          >
            {t("sonar.home.allHistory")}
          </button>
        </div>
        <HistoryList limit={4} />
      </section>
    </div>
  );
};
