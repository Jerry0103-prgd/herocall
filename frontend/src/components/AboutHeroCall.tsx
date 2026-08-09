type ReleaseNote = {
  version: string;
  date: string;
  summary?: string;
  groups: Array<{ title: string; items: string[] }>;
};

const releaseNotes: ReleaseNote[] = [
  {
    version: "V1.0.1",
    date: "2026-08-09",
    summary: "体验优化与问题修复",
    groups: [
      { title: "更新", items: [
        "UI视觉优化",
        "关注标的定位调整",
        "导航优化",
        "数据展示优化",
        "问题修复",
      ] },
    ],
  },
  {
    version: "V1.0.0",
    date: "2026-08-09",
    summary: "正式版本发布",
    groups: [
      { title: "新增", items: [
        "手动市场快照",
        "东方财富公告资讯",
        "AI复盘真实链路",
        "DeepSeek AI分析",
        "数据状态监控",
      ] },
      { title: "优化", items: [
        "Hero Call品牌升级",
        "UI统一优化",
        "本地数据安全存储",
      ] },
    ],
  },
  {
    version: "V0.9.0",
    date: "2026-08",
    groups: [
      { title: "主要更新", items: [
        "完成 Release 构建",
        "完成 Keychain 配置",
        "完成 AI复盘基础能力",
      ] },
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
              {release.summary ? <p>{release.summary}</p> : null}
              {release.groups.map((group) => <div className="release-note-group" key={group.title}>
                <h4>{group.title}</h4>
                <ul>{group.items.map((item) => <li key={item}>{item}</li>)}</ul>
              </div>)}
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
