// The parts drawn with React, for app.js to put where it wants: each draws into an element app.js made and returns how
// to take it down. app.js reaches them as window.BloxkeepUI (ui.js loads before it). The small pages (the tray's, a
// macro's panel, the mini window, the overlay, the message box, the browser's top) have bundles of their own: pages/*.
import { createRef } from 'react';
import { flushSync } from 'react-dom';
import { AccountForm, type AccountFormApi } from './AccountForm';
import { Feedback, type FeedbackApi } from './Feedback';
import type { Host } from './host';
import { ImportVault, type ImportResult, type ImportVaultApi } from './ImportVault';
import { JoinPlayer, type JoinPlayerApi } from './JoinPlayer';
import { Boot, Lock, type BootApi, type LockMode } from './Lock';
import { MultiHelp } from './MultiHelp';
import { LaunchOrder, type OrderAccount, type OrderListApi } from './OrderList';
import { Palette, type PaletteEntry } from './Palette';
import { PasswordLogin, type PasswordLoginApi } from './PasswordLogin';
import { PickList, type PickItem, type PickListApi } from './PickList';
import { PresetForm, type PresetBase, type PresetFormApi } from './PresetForm';
import { MacroEditor, type MacroEditorApi } from './MacroEditor';
import type { Macro } from './macros';
import { QuickLogin, type QuickLoginApi } from './QuickLogin';
import { Servers } from './Servers';
import { AskBody, DiscordLinkBody, ErrorsBody, MenuItems, ModalFrame, SaveServerBody, Toasts, WhatsNewTitle, type MenuItem, type ModalOpts, type ToastsApi } from './Overlays';
import { Shell, type SettingsPageInfo } from './Shell';
import { type StatsApi } from './StatsView';
import { WhatsNew, type Release } from './WhatsNew';
import { mount } from './pages/mount';
import { GamePage, type GameDetails } from './GamePage';

const BloxkeepUI = {
  /** A dialog's frame into its overlay (app.js's modal); its body a part drawn here, or empty for one put in later. */
  modalFrame(el: HTMLElement, host: Host, opts: ModalOpts) {
    return mount(el, host, <ModalFrame {...opts} />, false);
  },
  /** Bodies and a title for modalFrame: a question (with "Don't ask again"), errors, the Discord link box, What's new's title. */
  ask: (message: string, remember: boolean) => <AskBody message={message} remember={remember} />,
  errors: (errors: string[]) => <ErrorsBody errors={errors} />,
  discordLink: () => <DiscordLinkBody />,
  saveServer: (game: string, hasLink: boolean) => <SaveServerBody game={game} hasLink={hasLink} />,
  whatsNewTitle: (sub: string) => <WhatsNewTitle sub={sub} />,
  /** A menu's items into the menu app.js made; pick(i, byKeyboard) when one is chosen. */
  menu(el: HTMLElement, host: Host, items: MenuItem[], pick: (i: number, byKeyboard: boolean, arg?: string) => void, split: boolean, icons = false) {
    return mount(el, host, <MenuItems items={items} pick={pick} split={split} icons={icons} />, false);
  },
  /** The messages in the corner into #toasts (once, for the page's life). */
  toasts(el: HTMLElement, host: Host) {
    const api = createRef<ToastsApi>();
    const unmount = mount(el, host, <Toasts api={api} />, false);
    return { unmount, api: api.current! };
  },
  /** Runs fn (app.js telling the parts its state changed) and draws what that changes at once, before returning. */
  flush(fn: () => void) {
    flushSync(fn);
  },
  gamePage(el: HTMLElement, host: Host, game: GameDetails, close?: () => void) {
    return mount(el, host, <GamePage game={game} close={close} />);
  },
  whatsNew(el: HTMLElement, host: Host, releases: Release[], close: () => void) {
    return mount(el, host, <WhatsNew releases={releases} close={close} />);
  },
  feedback(el: HTMLElement, host: Host, locked: boolean) {
    const form = createRef<FeedbackApi>();
    const unmount = mount(el, host, <Feedback locked={locked} api={form} />);
    return { unmount, form: form.current! };
  },
  joinPlayer(el: HTMLElement, host: Host, count: number, submit: () => void) {
    const form = createRef<JoinPlayerApi>();
    const unmount = mount(el, host, <JoinPlayer count={count} submit={submit} api={form} />);
    return { unmount, form: form.current! };
  },
  /** Multi-Roblox's help into its dialog's body; enable turns it on (app.js closes the dialog first). */
  multiHelp(el: HTMLElement, host: Host, enable: () => void) {
    return mount(el, host, <MultiHelp enable={enable} />);
  },

  /** Launch order's text and list into its dialog's body; Done reads list.ids(). */
  launchOrder(el: HTMLElement, host: Host, accounts: OrderAccount[]) {
    const list = createRef<OrderListApi>();
    const unmount = mount(el, host, <LaunchOrder accounts={accounts} api={list} />);
    return { unmount, list: list.current! };
  },
  presetForm(el: HTMLElement, host: Host, opts: { base: PresetBase; isNew: boolean; accounts: OrderAccount[]; ticked: string[]; gone: number }) {
    const form = createRef<PresetFormApi>();
    const unmount = mount(el, host, <PresetForm {...opts} api={form} />);
    return { unmount, form: form.current! };
  },
  /** The macro form into its dialog's body: Save reads form.collect(). */
  macroEditor(el: HTMLElement, host: Host, opts: { base: Macro; isNew: boolean; accounts: { id: string; label: string }[]; macros: Macro[] }) {
    const form = createRef<MacroEditorApi>();
    const unmount = mount(el, host, <MacroEditor {...opts} api={form} />);
    return { unmount, form: form.current! };
  },
  /** A game's servers into the Servers dialog; chosen picks a server by its job, chosenPrivate a private server by its access code. */
  servers(el: HTMLElement, host: Host, placeId: number, ids: string[], chosen: (placeId: number, jobId: string) => void,
    chosenPrivate: (placeId: number, accessCode: string) => void) {
    return mount(el, host, <Servers placeId={placeId} ids={ids} chosen={chosen} chosenPrivate={chosenPrivate} />);
  },
  passwordLogin(el: HTMLElement, host: Host, forRealm: string | null, way: (way: string, realm: string) => void) {
    const api = createRef<PasswordLoginApi>();
    const unmount = mount(el, host, <PasswordLogin forRealm={forRealm} api={api} way={way} />);
    return { unmount, api: api.current! };
  },
  /** Quick Login into its dialog's body; browserButton is the footer's "Sign in with the browser" (roblox.com only). */
  quickLogin(el: HTMLElement, host: Host, forRealm: string | null, forName: string, browserButton: () => HTMLElement | null) {
    const api = createRef<QuickLoginApi>();
    const unmount = mount(el, host, <QuickLogin forRealm={forRealm} forName={forName} api={api} browserButton={browserButton} />);
    return { unmount, api: api.current! };
  },

  palette(el: HTMLElement, host: Host, entries: PaletteEntry[], close: () => void) {
    return mount(el, host, <Palette entries={entries} close={close} />);
  },
  /** Importing from a vault file into its dialog's body: the main button calls api.next (primary() is that button). */
  importVault(el: HTMLElement, host: Host, primary: () => HTMLButtonElement | null, finished: (r: ImportResult) => void) {
    const api = createRef<ImportVaultApi>();
    const unmount = mount(el, host, <ImportVault api={api} primary={primary} finished={finished} />);
    return { unmount, api: api.current! };
  },
  accountForm(el: HTMLElement, host: Host, account: object | null, browser: () => void) {
    const form = createRef<AccountFormApi>();
    const unmount = mount(el, host, <AccountForm account={account as never} api={form} browser={browser} />);
    return { unmount, form: form.current! };
  },

  /** The main page into #app (the vault open); stats is the Statistics tab's api. */
  shell(el: HTMLElement, host: Host, opts: { pages: SettingsPageInfo[]; page: string; preview: boolean }) {
    const stats = createRef<StatsApi>();
    const unmount = mount(el, host, <Shell {...opts} stats={stats} />);
    return { unmount, stats: stats.current! };
  },
  /** The lock screen into #app: opened gets the state of the vault it opened (app.js applyState). */
  lock(el: HTMLElement, host: Host, opts: { mode: LockMode; notice: string; kept: { pw1: string; pw2: string } | null }, opened: (state: object) => void) {
    return mount(el, host, <Lock {...opts} opened={opened} />);
  },
  /** The loading screen into #app; app.js moves its bar (api.paint) as the app's steps go. */
  boot(el: HTMLElement, host: Host) {
    const api = createRef<BootApi>();
    const unmount = mount(el, host, <Boot api={api} />);
    return { unmount, api: api.current! };
  },
  /** A list to tick into a picker's body; the footer's buttons read list.values(). */
  pickList(el: HTMLElement, host: Host, text: string, items: PickItem[]) {
    const list = createRef<PickListApi>();
    const unmount = mount(el, host, <PickList text={text} items={items} api={list} />);
    return { unmount, list: list.current! };
  },
};

declare global {
  interface Window { BloxkeepUI: typeof BloxkeepUI }
}
window.BloxkeepUI = BloxkeepUI;
