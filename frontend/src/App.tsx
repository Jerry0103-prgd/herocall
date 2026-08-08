import { useState } from "react";

import { Sidebar, type PageId } from "./components/Sidebar";
import { ComingSoonPage } from "./pages/ComingSoonPage";
import { DashboardPage } from "./pages/DashboardPage";

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
      {activePage === "dashboard" ? (
        <DashboardPage />
      ) : (
        <ComingSoonPage title={pageTitles[activePage]} />
      )}
    </main>
  );
}

export default App;
