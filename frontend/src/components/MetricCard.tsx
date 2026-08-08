type MetricCardProps = {
  label: string;
  value: string | null;
  tone?: "neutral" | "positive" | "negative";
};

export function MetricCard({ label, value, tone = "neutral" }: MetricCardProps) {
  return (
    <article className="metric-card">
      <p>{label}</p>
      <strong className={`metric-value metric-value--${tone}`}>{value ?? "暂无数据"}</strong>
    </article>
  );
}
