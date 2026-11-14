import React, { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import SecureInputWarning from "@/components/SecureInputWarning";
import { FileDropOverlay } from "@/components/FileDropOverlay";
import { Mark } from "./parts";
import { HomeView } from "./HomeView";
import { HistoryList } from "./HistoryList";
import { SettingsView } from "./SettingsView";

type Tab = "home" | "history" | "settings";

export const SonarShell: React.FC<{ dir: "ltr" | "rtl" }> = ({ dir }) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [tab, setTab] = useState<Tab>("home");
  const [search, setSearch] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const cloud = !!getSetting("cloud_transcription_enabled");

  useLayoutEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 });
  }, [tab]);

  const tabs: { id: Tab; label: string }[] = [
    { id: "home", label: t("sonar.tabs.home") },
    { id: "history", label: t("sonar.tabs.history") },
    { id: "settings", label: t("sonar.tabs.settings") },
  ];

  return (
    <div dir={dir} className="sonar-root">
      <header className="sonar-top">
        <div className="sonar-brand">
          <Mark size={30} />
          <span className="sonar-wordmark">{t("sonar.brand")}</span>
        </div>
        <nav className="sonar-tabs" role="tablist">
          {tabs.map((x) => (
            <button
              key={x.id}
              type="button"
              role="tab"
              aria-selected={tab === x.id}
              className={tab === x.id ? "on" : ""}
              onClick={() => setTab(x.id)}
            >
              {x.label}
            </button>
          ))}
        </nav>
        <span className={`sonar-pill ${cloud ? "cloud" : "offline"}`}>
          <span className="dot" />
          {cloud ? t("sonar.mode.cloud") : t("sonar.mode.offline")}
        </span>
      </header>

      <main ref={scrollRef} className="sonar-main">
        <SecureInputWarning />
        {tab === "home" && <HomeView onOpenHistory={() => setTab("history")} />}
        {tab === "history" && (
          <div className="sonar-page">
            <div className="sonar-card-head">
              <h1 className="sonar-h1">{t("sonar.tabs.history")}</h1>
              <input
                type="search"
                className="sonar-search"
                value={search}
                placeholder={t("sonar.history.search")}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
            <HistoryList search={search} />
          </div>
        )}
        {tab === "settings" && <SettingsView />}
      </main>
      <FileDropOverlay onDropped={() => setTab("history")} />
    </div>
  );
};
