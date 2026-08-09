import type { ReactNode } from "react";

export type DataStatusTone = "success" | "empty" | "failed";

type DataStatusCardProps = {
  title: string;
  tone: DataStatusTone;
  headline: string;
  children?: ReactNode;
};

export function DataStatusCard({ title, tone, headline, children }: DataStatusCardProps) {
  const symbol = tone === "success" ? "✓" : tone === "failed" ? "!" : "–";
  return (
    <article className={`data-status-card data-status-card--${tone}`}>
      <p>{title}</p>
      <strong><span aria-hidden="true">{symbol}</span>{headline}</strong>
      {children ? <dl>{children}</dl> : null}
    </article>
  );
}
