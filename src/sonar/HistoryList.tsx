import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events, type HistoryEntry } from "@/bindings";
import { copyToClipboard } from "@/components/settings/history/clipboard";
import { relativeTime } from "./parts";

const PAGE = 30;

export const entryText = (e: HistoryEntry) =>
  (e.post_processed_text || e.transcription_text || "").trim();

/** Dictations, newest first. `limit` shows a fixed preview with no paging. */
export const HistoryList: React.FC<{ limit?: number; search?: string }> = ({
  limit,
  search = "",
}) => {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState<number | null>(null);

  const load = useCallback(
    async (cursor: number | null) => {
      setLoading(true);
      const res = await commands.getHistoryEntries(cursor, limit ?? PAGE);
      if (res.status === "ok") {
        setEntries((prev) =>
          cursor === null ? res.data.entries : [...prev, ...res.data.entries],
        );
        setHasMore(!limit && res.data.has_more);
      }
      setLoading(false);
    },
    [limit],
  );

  useEffect(() => {
    load(null);
  }, [load]);

  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen(({ payload }) => {
      if (payload.action === "added") {
        setEntries((prev) => {
          const next = [
            payload.entry,
            ...prev.filter((e) => e.id !== payload.entry.id),
          ];
          return limit ? next.slice(0, limit) : next;
        });
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      } else if (payload.action === "deleted") {
        setEntries((prev) => prev.filter((e) => e.id !== payload.id));
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [limit]);

  const copy = async (e: HistoryEntry) => {
    if (await copyToClipboard(entryText(e))) {
      setCopied(e.id);
      window.setTimeout(() => setCopied(null), 1400);
    }
  };

  const remove = async (id: number) => {
    setEntries((prev) => prev.filter((e) => e.id !== id));
    await commands.deleteHistoryEntry(id);
  };

  const q = search.trim().toLowerCase();
  const shown = q
    ? entries.filter((e) => entryText(e).toLowerCase().includes(q))
    : entries;

  if (!loading && shown.length === 0) {
    return (
      <p className="sonar-empty">
        {q ? t("sonar.history.noMatch") : t("sonar.history.empty")}
      </p>
    );
  }

  return (
    <div className="sonar-history">
      {shown.map((e) => {
        const text = entryText(e);
        return (
          <article key={e.id} className="sonar-entry">
            <p className={text ? "sonar-entry-text" : "sonar-entry-text muted"}>
              {text || t("sonar.history.noSpeech")}
            </p>
            <div className="sonar-entry-meta">
              <span className="sonar-muted">
                {relativeTime(e.timestamp, i18n.language)}
              </span>
              <span className="sonar-entry-actions">
                {text && (
                  <button
                    type="button"
                    className="sonar-btn sonar-btn-ghost"
                    onClick={() => copy(e)}
                  >
                    {copied === e.id
                      ? t("sonar.history.copied")
                      : t("sonar.history.copy")}
                  </button>
                )}
                {!limit && (
                  <button
                    type="button"
                    className="sonar-btn sonar-btn-ghost danger"
                    onClick={() => remove(e.id)}
                  >
                    {t("sonar.history.delete")}
                  </button>
                )}
              </span>
            </div>
          </article>
        );
      })}
      {hasMore && (
        <button
          type="button"
          className="sonar-btn sonar-btn-quiet sonar-more"
          disabled={loading}
          onClick={() => load(entries[entries.length - 1]?.id ?? null)}
        >
          {t("sonar.history.more")}
        </button>
      )}
    </div>
  );
};
