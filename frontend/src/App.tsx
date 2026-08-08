import { useState } from "react";

import { Sidebar, type PageId } from "./components/Sidebar";
import { ComingSoonPage } from "./pages/ComingSoonPage";
import { DashboardPage } from "./pages/DashboardPage";
import { NewsPage } from "./pages/NewsPage";
import { PortfolioPage } from "./pages/PortfolioPage";
import { SettingsPage } from "./pages/SettingsPage";

const pageTitles: Record<Exclude<PageId, "dashboard">, string> = {
  news: "财经资讯",
  review: "仓位复盘",
  holdings: "我的持仓",
  calendar: "事件日历",
  settings: "设置",
};

function App() {
  const [activePage, setActivePage] = useState<PageId>("dashboard");

  return (
    <main className="app-shell">
      <Sidebar activePage={activePage} onNavigate={setActivePage} />
      {activePage === "dashboard" ? <DashboardPage /> : null}
      {activePage === "news" ? <NewsPage /> : null}
      {activePage === "holdings" ? <PortfolioPage /> : null}
      {activePage === "settings" ? <SettingsPage /> : null}
      {activePage !== "dashboard" && activePage !== "news" && activePage !== "holdings" && activePage !== "settings" ? <ComingSoonPage title={pageTitles[activePage]} /> : null}
    </main>
  );
}

export default App;
