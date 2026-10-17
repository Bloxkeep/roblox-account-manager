// The main page once the vault is open: the sidebar, and the tabs beside it (Accounts with its dock, Presets, the game
// monitor, Statistics, Settings). app.js builds it once per language (buildShell) and shows one tab at a time (setView).
import { type Ref } from 'react';
import { AccountGrid, AccountsBar, AccountsHead, Dock } from './Accounts';
import { Icon, useHost } from './host';
import { PresetsView } from './PresetsView';
import { MonitorHead, Sidebar } from './Sidebar';
import { AppearancePage } from './settings/AppearancePage';
import { BackupPage } from './settings/BackupPage';
import { DiscordPage } from './settings/DiscordPage';
import { LaunchPage } from './settings/LaunchPage';
import { NotifyPage } from './settings/NotifyPage';
import { PerfPage } from './settings/PerfPage';
import { RobloxPage } from './settings/RobloxPage';
import { SecurityPage } from './settings/SecurityPage';
import { UpdatesPage } from './settings/UpdatesPage';
import { StatsView, type StatsApi } from './StatsView';

export type SettingsPageInfo = { id: string; icon: string; title: string };

export function Shell({ pages, page, preview, stats }: { pages: SettingsPageInfo[]; page: string; preview: boolean; stats: Ref<StatsApi> }) {
  const { t } = useHost();
  return <div className="shell">
    <Sidebar />
    <main className="main">
      <section className="view" id="view-accounts">
        <header className="top">
          <div className="top-title"><h1>{t('acc.title')}</h1><AccountsHead /></div>
          <div className="top-tools">
            <label className="search"><Icon name="search" /><input id="q" placeholder={t('acc.search')} maxLength={100} aria-label={t('acc.search')} /></label>
            <button className="icon-btn" data-act="refresh-presence" title={t('acc.refreshTip')}><Icon name="refresh" /></button>
            <button className="icon-btn" data-act="tools-menu" title={t('acc.moreTip')} aria-haspopup="menu"><Icon name="more" /></button>
            <div className="split">
              <button className="btn primary" data-act="add-choose"><Icon name="userPlus" /><span>{t('acc.add')}</span></button>
              <button className="btn primary" data-act="add-menu" title={t('acc.addMoreTip')} aria-haspopup="menu"><Icon name="chevronDown" /></button>
            </div>
          </div>
        </header>
        <AccountsBar />
        <div className="scroll" id="scroll"><AccountGrid /></div>
        <Dock />
      </section>
      <section className="view" id="view-presets" hidden><PresetsView /></section>
      <section className="view" id="view-stats" hidden><StatsView api={stats} /></section>
      <section className="view" id="view-monitor" hidden>
        <header className="top"><div className="top-title"><h1>{t('mon.title')}</h1><MonitorHead /></div></header>
        {/* In the browser preview there is no app to draw the pictures: it says what goes here. */}
        <div className={`mon-area ${preview ? 'preview' : ''}`} id="mon-area">{preview ? <p>{t('mon.preview')}</p> : null}</div>
      </section>
      <section className="view" id="view-settings" hidden><Settings pages={pages} page={page} /></section>
    </main>
  </div>;
}

/**
 * Settings: the list of pages on the left (a search box over all of them) and the chosen page on the right. app.js
 * shows a page and searches (openSettings, searchSettings); the pages are React's (ui/src/settings).
 */
function Settings({ pages, page }: { pages: SettingsPageInfo[]; page: string }) {
  const { t } = useHost();
  const body = (id: string, title: string) => {
    switch (id) {
      case 'launch': return <LaunchPage title={title} />;
      case 'roblox': return <RobloxPage title={title} />;
      case 'perf': return <PerfPage title={title} />;
      case 'notify': return <NotifyPage title={title} />;
      case 'discord': return <DiscordPage title={title} />;
      case 'appearance': return <AppearancePage title={title} />;
      case 'security': return <SecurityPage title={title} />;
      case 'backup': return <BackupPage title={title} />;
      case 'updates': return <UpdatesPage title={title} />;
      default: return null;
    }
  };
  return <>
    <header className="top"><div className="top-title"><h1>{t('set.title')}</h1></div></header>
    <div className="set-layout">
      <nav className="set-nav" aria-label={t('set.title')}>
        <label className="search set-search"><Icon name="search" /><input id="set-q" type="search" placeholder={t('set.search')} maxLength={60} aria-label={t('set.search')} autoComplete="off" spellCheck={false} /></label>
        {pages.map(p => <button key={p.id} type="button" className="set-nav-item" data-set-page={p.id} aria-current={p.id === page ? 'page' : undefined}>
          <Icon name={p.icon} /><span>{p.title}</span><i className="set-nav-dot" title={t('set.needsLook')} hidden /></button>)}
      </nav>
      <div className="scroll set-scroll"><div className="settings">
        {pages.map(p => <section key={p.id} className="set-page" data-page={p.id} id={`set-page-${p.id}`} aria-labelledby={`set-page-${p.id}-h`} hidden={p.id !== page}>{body(p.id, p.title)}</section>)}
        <p className="set-empty" id="set-empty" hidden>{t('set.noMatch')}</p>
      </div></div>
    </div>
  </>;
}
