import React from "react";
import { cx } from "./utils";

export function StatusPill({ active, label }: { active: boolean; label: string }) {
  return <span className={cx("pill", active ? "pill-ok" : "pill-muted")}>{label}</span>;
}

export function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function StatCard({ icon, label, value, ok }: { icon: React.ReactNode; label: string; value: React.ReactNode; ok?: boolean }) {
  return (
    <div className="stat-card">
      <div className={cx("stat-icon", ok ? "stat-icon-ok" : undefined)}>{icon}</div>
      <div>
        <p>{label}</p>
        <strong>{value}</strong>
      </div>
    </div>
  );
}

export function Avatar({ name }: { name: string }) {
  const initials = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((s) => s[0]?.toUpperCase())
    .join("") || "P";
  return <div className="provider-avatar">{initials}</div>;
}

export function OpenAIIcon() {
  return (
    <div className="openai-avatar" aria-label="OpenAI Official">
      <svg viewBox="0 0 64 64" width="30" height="30" role="img">
        <path
          d="M31.6 6.5c4.4 0 8.3 2.3 10.5 5.8 4.1.2 8 2.5 10.2 6.3 2.2 3.8 2.1 8.4.2 11.9 1.9 3.7 1.9 8.2-.3 12-2.2 3.8-6.1 6.1-10.2 6.3-2.2 3.5-6.1 5.7-10.5 5.7-4.4 0-8.3-2.2-10.5-5.7-4.1-.2-8-2.5-10.2-6.3-2.2-3.8-2.2-8.3-.3-12-1.9-3.6-1.9-8.1.3-11.9 2.2-3.8 6.1-6.1 10.2-6.3 2.2-3.5 6.1-5.8 10.6-5.8Zm0 5.6c-2.3 0-4.3 1-5.7 2.7l12.1 7V16c0-2.2-2.9-3.9-6.4-3.9Zm11 6.1v14l5-2.9c1.9-1.1 2.1-4.5.4-7.5-1.2-2.2-3.2-3.5-5.4-3.6Zm-23.9.1c-2.1.2-4.1 1.5-5.3 3.6-1.8 3-1.6 6.4.4 7.5l5 2.9v-14Zm5.2 1.2v14.1l7.7 4.5 7.7-4.5V19.5l-7.7 4.5-7.7-4.5Zm-9.2 15.9c-1.9 1.2-2.1 4.5-.4 7.5 1.2 2.1 3.2 3.4 5.3 3.6v-14l-4.9 2.9Zm34 .1-5 2.9v14c2.1-.2 4.1-1.5 5.3-3.6 1.8-3 1.6-6.4-.3-7.3Zm-17.1 8.7-7.7-4.5v5.8c0 2.2 2.9 3.9 6.4 3.9 2.3 0 4.4-1 5.7-2.7l-4.4-2.5Z"
          fill="currentColor"
        />
      </svg>
    </div>
  );
}

export function JsonPreview({ text }: { text: string }) {
  return (
    <pre className="toml-preview json-preview" aria-label="JSON preview">
      {text.split("\n").map((line, index) => (
        <div className="toml-line" key={index}>
          <span className="toml-line-no">{index + 1}</span>
          <code>{line}</code>
        </div>
      ))}
    </pre>
  );
}

function renderTomlValue(value: string, lineKey: string) {
  const parts = value.split(/("(?:\\.|[^"])*")/g);
  return parts.map((part, index) => {
    if (!part) return null;
    const key = `${lineKey}-v-${index}`;
    if (/^"(?:\\.|[^"])*"$/.test(part)) {
      return <span className="toml-string" key={key}>{part}</span>;
    }
    const boolParts = part.split(/\b(true|false)\b/g);
    return boolParts.map((piece, boolIndex) => {
      if (piece === "true" || piece === "false") {
        return <span className="toml-bool" key={`${key}-b-${boolIndex}`}>{piece}</span>;
      }
      return <React.Fragment key={`${key}-t-${boolIndex}`}>{piece}</React.Fragment>;
    });
  });
}

function renderTomlLine(line: string, index: number) {
  const key = `toml-${index}`;
  if (line.trim().startsWith("#")) {
    return <span className="toml-comment">{line}</span>;
  }
  if (/^\s*\[[^\]]+\]\s*$/.test(line)) {
    return <span className="toml-section">{line}</span>;
  }
  const eqIndex = line.indexOf("=");
  if (eqIndex > -1) {
    const left = line.slice(0, eqIndex);
    const right = line.slice(eqIndex + 1);
    return (
      <>
        <span className="toml-key">{left}</span>
        <span className="toml-eq">=</span>
        {renderTomlValue(right, key)}
      </>
    );
  }
  return <>{line}</>;
}

export function TomlPreview({ text }: { text: string }) {
  return (
    <pre className="toml-preview" aria-label="TOML preview">
      {text.split("\n").map((line, index) => (
        <div className="toml-line" key={index}>
          <span className="toml-line-no">{index + 1}</span>
          <code>{renderTomlLine(line, index)}</code>
        </div>
      ))}
    </pre>
  );
}
