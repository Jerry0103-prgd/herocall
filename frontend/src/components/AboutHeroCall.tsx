type ReleaseNote = {
  version: string;
  date: string;
  items: string[];
};

const releaseNotes: ReleaseNote[] = [
  {
    version: "V1.0.0",
    date: "2026-08-09",
    items: [
      "完成 Hero Call 品牌升级",
      "接入 DeepSeek AI 复盘",
      "支持东方财富公告资讯",
      "支持事件日历",
      "增加数据状态概览",
      "完成手动市场快照模式",
    ],
  },
  {
    version: "V0.9.0",
    date: "2026-08",
    items: [
      "完成 Release 构建",
      "完成 Keychain 配置",
      "完成 AI复盘基础能力",
    ],
  },
];

type AboutHeroCallProps = {
  version: string | null;
};

export function AboutHeroCall({ version }: AboutHeroCallProps) {
  return (
    <section className="settings-section about-section" aria-labelledby="about-title">
      <div className="section-heading">
        <div><p className="section-kicker">About</p><h2 id="about-title">关于 Hero Call</h2></div>
      </div>
      <div className="settings-card about-card">
        <div className="about-product">
          <strong>Hero Call</strong>
          <p>AI Portfolio Assistant</p>
          <span>当前版本：{version ? `v${version}` : "暂无数据"}</span>
        </div>
        <div className="release-notes" aria-label="迭代记录">
          <h3>迭代记录</h3>
          {releaseNotes.map((release) => (
            <article className="release-note" key={release.version}>
              <header><strong>{release.version}</strong><time>{release.date}</time></header>
              <ul>{release.items.map((item) => <li key={item}>{item}</li>)}</ul>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
