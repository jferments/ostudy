import { useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  flushNativeDiagnostics,
  getDiagnosticConfiguration,
  setDiagnosticFilter,
  type DiagnosticConfiguration,
  type DiagnosticLevel,
} from "../../ipc/diagnostics";
import styles from "./Settings.module.css";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

type DiagnosticTarget =
  | "all"
  | "database"
  | "ai"
  | "qr"
  | "device_sync"
  | "documents"
  | "frontend";

const TARGET_OPTIONS: ReadonlyArray<{ value: DiagnosticTarget; label: string; target?: string }> = [
  { value: "all", label: "All ostudy subsystems" },
  { value: "database", label: "Database and migrations", target: "ostudy.database" },
  { value: "ai", label: "AI providers and document generation", target: "ostudy.ai" },
  { value: "qr", label: "QR scanning and pairing", target: "ostudy.qr" },
  { value: "device_sync", label: "Device synchronization", target: "ostudy.device_sync" },
  { value: "documents", label: "Documents, rendering, and OCR", target: "ostudy.documents" },
  { value: "frontend", label: "Webview and frontend", target: "ostudy.frontend" },
];

function filterFor(target: DiagnosticTarget, level: DiagnosticLevel): string {
  const option = TARGET_OPTIONS.find((entry) => entry.value === target);
  if (!option?.target) return level;
  return `warn,${option.target}=${level}`;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Unknown";
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export function DiagnosticsSettings({ showToast }: { showToast: ShowToast }) {
  const [configuration, setConfiguration] = useState<DiagnosticConfiguration | null>(null);
  const [target, setTarget] = useState<DiagnosticTarget>("database");
  const [level, setLevel] = useState<Extract<DiagnosticLevel, "debug" | "trace">>("debug");
  const [durationSeconds, setDurationSeconds] = useState(15 * 60);
  const [busy, setBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const proposedFilter = useMemo(() => filterFor(target, level), [level, target]);

  useEffect(() => {
    let active = true;
    void getDiagnosticConfiguration()
      .then((next) => {
        if (active) setConfiguration(next);
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      });
    return () => {
      active = false;
    };
  }, []);

  async function applyTemporaryFilter(): Promise<void> {
    setBusy(true);
    setErrorMessage(null);
    try {
      const next = await setDiagnosticFilter(proposedFilter, durationSeconds);
      setConfiguration(next);
      showToast("success", "Temporary diagnostics enabled.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function restoreNormalFilter(): Promise<void> {
    setBusy(true);
    setErrorMessage(null);
    try {
      const next = await setDiagnosticFilter("info");
      setConfiguration(next);
      showToast("success", "Normal diagnostics restored.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function flushNow(): Promise<void> {
    setBusy(true);
    setErrorMessage(null);
    try {
      const flushed = await flushNativeDiagnostics();
      if (!flushed) throw new Error("The diagnostic writer did not acknowledge the flush.");
      showToast("success", "Diagnostic log flushed.");
      setConfiguration(await getDiagnosticConfiguration());
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={styles.panelGrid} aria-busy={busy}>
      {errorMessage ? (
        <InlineNotice kind="error" className={styles.fullWidth}>{errorMessage}</InlineNotice>
      ) : null}

      <section className={styles.settingsCard}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>Temporary diagnostic session</h2>
            <p>
              Increase detail for one subsystem while reproducing a problem. The previous filter is
              restored automatically when the session expires.
            </p>
          </div>
        </div>

        <div className={styles.settingControl}>
          <label>
            Subsystem
            <select
              value={target}
              disabled={busy}
              onChange={(event) => setTarget(event.currentTarget.value as DiagnosticTarget)}
            >
              {TARGET_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Detail level
            <select
              value={level}
              disabled={busy}
              onChange={(event) =>
                setLevel(event.currentTarget.value as Extract<DiagnosticLevel, "debug" | "trace">)
              }
            >
              <option value="debug">Debug — decisions, counts, and durations</option>
              <option value="trace">Trace — high-detail subsystem internals</option>
            </select>
          </label>
          <label>
            Automatic expiry
            <select
              value={durationSeconds}
              disabled={busy}
              onChange={(event) => setDurationSeconds(Number(event.currentTarget.value))}
            >
              <option value={5 * 60}>5 minutes</option>
              <option value={15 * 60}>15 minutes</option>
              <option value={30 * 60}>30 minutes</option>
              <option value={60 * 60}>1 hour</option>
            </select>
          </label>
        </div>

        <p className={styles.note}>
          Filter preview: <code>{proposedFilter}</code>
        </p>
        <div className="header-actions">
          <Button
            type="button"
            variant="primary"
            disabled={busy}
            onClick={() => void applyTemporaryFilter()}
          >
            Enable temporary diagnostics
          </Button>
          <Button
            type="button"
            variant="secondary"
            disabled={busy}
            onClick={() => void restoreNormalFilter()}
          >
            Restore normal diagnostics
          </Button>
        </div>
      </section>

      <section className={styles.settingsCard}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>Current diagnostic state</h2>
            <p>Diagnostics stay on this device and use bounded rotating files.</p>
          </div>
        </div>
        <dl>
          <dt>Active filter</dt>
          <dd>
            <code>{configuration?.filter ?? "Loading…"}</code>
          </dd>
          <dt>Current log</dt>
          <dd>{configuration?.logPath ?? "Unavailable until application logging initializes"}</dd>
          <dt>Maximum file size</dt>
          <dd>{configuration ? formatBytes(configuration.maximumFileBytes) : "Loading…"}</dd>
          <dt>Dropped queued events</dt>
          <dd>{configuration?.droppedEvents ?? 0}</dd>
        </dl>
        <Button
          type="button"
          variant="secondary"
          disabled={busy || configuration === null}
          onClick={() => void flushNow()}
        >
          Flush diagnostic log
        </Button>
      </section>

      <section className={styles.fullWidthCard}>
        <h2>Privacy and storage limits</h2>
        <p className={styles.securityNote}>
          ostudy never uploads diagnostics automatically. API credentials, authorization headers,
          prompts, model responses, document or OCR text, flashcard content, SQL parameters, and QR
          payloads are prohibited diagnostic fields and are centrally redacted. The writer keeps only
          the current and previous bounded log files; logging is never allowed to block database or
          study operations.
        </p>
      </section>
    </div>
  );
}
