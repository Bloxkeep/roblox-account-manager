// A part of the page that fails to draw (a bug, a state it didn't expect) shows a short note in its place instead of taking
// the whole page with it: React would otherwise take down everything drawn in the same root (the sidebar, Lock…). It tries
// again with the app's next state or when "Try again" is pressed, and says what went wrong in the console.
import { Component, type ReactNode } from 'react';
import { useAppState, useHost } from './host';

class Guard extends Component<{ name: string; retry: unknown; said: string; again: string; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(error: unknown) { console.error(`Bloxkeep: ${this.props.name} stopped drawing.`, error); }
  componentDidUpdate(prev: { retry: unknown }) { if (this.state.failed && prev.retry !== this.props.retry) this.setState({ failed: false }); }
  render() {
    if (!this.state.failed) return this.props.children;
    return <div className="note bad guard-note" role="alert"><span>{this.props.said}</span>
      <button type="button" className="btn sm" onClick={() => this.setState({ failed: false })}>{this.props.again}</button></div>;
  }
}

export function Guarded({ name, children }: { name: string; children: ReactNode }) {
  const { t } = useHost();
  return <Guard name={name} retry={useAppState()} said={t('err.draw')} again={t('pw.retry')}>{children}</Guard>;
}
